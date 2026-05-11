use super::cdp::CdpEndpointManager;
use super::job::JobManager;
use super::types::{
    BatchLaunchResult, CdpEndpointResponse, EnvironmentStartRequest, WindowBoundsRequest,
};
use crate::app::{EventPublisher, Result, RuntimeError};
use crate::infrastructure::diagnostics::log_info;
use crate::infrastructure::eventbus::{LaunchConfig, Message, eventbus_manager};
use crate::infrastructure::eventbus::{Topic, get_eventbus_manager};
use crate::services::environment::{EnvironmentStatus, EnvironmentStatusManager};
use std::path::Path;
use std::sync::Arc;

pub async fn launch_browser(
    request: EnvironmentStartRequest,
    cdp_endpoint_manager: Arc<CdpEndpointManager>,
    job_manager: Arc<JobManager>,
    status_manager: Arc<EnvironmentStatusManager>,
    events: EventPublisher,
) -> Result<CdpEndpointResponse> {
    let path = Path::new(&request.exe_path);
    if !path.exists() {
        return Err(RuntimeError::Internal("可执行文件不存在".into()));
    }
    let work_dir = path
        .parent()
        .ok_or_else(|| RuntimeError::Internal("无法获取可执行文件所在目录".into()))?;

    let env_id = request.env_uuid.trim().to_string();
    let cdp_port = cdp_endpoint_manager.allocate_port(&env_id).await?;
    status_manager
        .set_status(&env_id, EnvironmentStatus::Initializing)
        .await;
    status_manager
        .set_status(&env_id, EnvironmentStatus::Starting)
        .await;

    let launch_config = LaunchConfig {
        env_uuid: env_id.clone(),
        user_data_dir: request.user_data_dir.clone(),
        proxy: request.proxy.clone(),
        kernel_version: None,
        extensions: None,
        custom_flags: None,
        cookies: request.cookies.clone(),
        urls: request.urls.clone(),
        fingerprint_config: request.fingerprint_config.clone(),
        accounts: request.accounts.clone(),
    };

    let _server = eventbus_manager().start_server(env_id.clone(), Some(launch_config));

    if let Err(error) = spawn_browser_process(
        &request.exe_path,
        work_dir,
        &env_id,
        &request.user_data_dir,
        cdp_port,
        request.display_id.as_deref(),
        request.window_position.as_deref(),
        request.window_size.as_deref(),
        request.extension_dirs.as_ref(),
        job_manager.clone(),
    )
    .await
    {
        cdp_endpoint_manager.remove(&env_id).await;
        let _ = events.emit(
            "environment.launch_failed",
            &serde_json::json!({
                "env_uuid": env_id,
                "error": error.to_string(),
            }),
        );
        return Err(error);
    }

    log_info(
        "kernel",
        format!("Browser launched for environment {}", env_id),
    );
    let _ = events.emit(
        "environment.launch_started",
        &serde_json::json!({
            "env_uuid": env_id,
            "cdp_port": cdp_port,
        }),
    );

    let endpoint = cdp_endpoint_manager
        .get_endpoint(&request.env_uuid)
        .await
        .ok_or_else(|| RuntimeError::Internal("failed to resolve cdp endpoint".into()))?;

    Ok(CdpEndpointResponse {
        env_uuid: endpoint.env_uuid,
        host: endpoint.host,
        port: endpoint.port,
        version_url: endpoint.version_url,
        list_url: endpoint.list_url,
        browser_ws_url: endpoint.browser_ws_url,
    })
}

async fn spawn_browser_process(
    exe_path: &str,
    work_dir: &Path,
    env_id: &str,
    user_data_dir: &str,
    cdp_port: u16,
    display_id: Option<&str>,
    window_position: Option<&str>,
    window_size: Option<&str>,
    extension_dirs: Option<&Vec<String>>,
    job_manager: Arc<JobManager>,
) -> Result<()> {
    let mut args = vec![
        format!("--simprint-env-id={}", env_id),
        format!("--user-data-dir={}", user_data_dir),
        format!("--remote-debugging-port={}", cdp_port),
        "--remote-allow-origins=*".to_string(),
        "--disable-skia-graphite".to_string(),
    ];

    if cfg!(debug_assertions) {
        args.push("--enable-logging".to_string());
        args.push("--v=1".to_string());
    }

    if let Some(id) = display_id {
        args.push(format!("--simprint-display-id={}", id));
    }
    if let Some(position) = window_position {
        args.push(format!("--window-position={}", position));
    }
    if let Some(size) = window_size {
        args.push(format!("--window-size={}", size));
    }
    if let Some(dirs) = extension_dirs {
        if !dirs.is_empty() {
            args.push(format!("--load-extension={}", dirs.join(",")));
            log_info(
                "kernel",
                format!("加载 {} 个扩展: {}", dirs.len(), dirs.join(", ")),
            );
        }
    }

    #[cfg(target_os = "windows")]
    {
        use crate::infrastructure::diagnostics::log_error;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let child = std::process::Command::new(exe_path)
            .current_dir(work_dir)
            .args(&args)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|error| RuntimeError::Internal(error.to_string()))?;

        let pid = child.id();
        let env_id_clone = env_id.to_string();
        tokio::spawn(async move {
            if let Err(error) = job_manager.create_and_assign(&env_id_clone, pid).await {
                log_error(
                    "kernel",
                    format!("Failed to assign process {} to Job Object: {}", pid, error),
                );
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = job_manager;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(exe_path)
            .map_err(|error| RuntimeError::Internal(error.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(exe_path, perms)
            .map_err(|error| RuntimeError::Internal(error.to_string()))?;
        std::process::Command::new(exe_path)
            .current_dir(work_dir)
            .args(&args)
            .spawn()
            .map_err(|error| RuntimeError::Internal(error.to_string()))?;
    }

    Ok(())
}

pub async fn stop_environment(
    env_uuid: String,
    cdp_endpoint_manager: Arc<CdpEndpointManager>,
    job_manager: Arc<JobManager>,
    status_manager: Arc<EnvironmentStatusManager>,
    events: EventPublisher,
) -> Result<()> {
    let env_id = env_uuid.trim().to_string();
    let manager = eventbus_manager();

    if !manager.is_connected(&env_id).await {
        return Err(RuntimeError::Internal(format!("环境 {} 未连接", env_id)));
    }

    manager.disconnect(&env_id).await?;
    job_manager.remove(&env_id).await;
    cdp_endpoint_manager.remove(&env_id).await;
    status_manager
        .set_status(&env_id, EnvironmentStatus::Stopped)
        .await;
    let _ = events.emit(
        "environment.stopped",
        &serde_json::json!({ "env_uuid": env_id }),
    );
    Ok(())
}

pub async fn refresh_proxy(
    env_uuid: String,
    proxy: Option<super::types::BrowserProxyConfigPayload>,
    events: EventPublisher,
) -> Result<()> {
    let env_id = env_uuid.trim().to_string();
    let manager = eventbus_manager();

    if !manager.is_connected(&env_id).await {
        return Err(RuntimeError::Internal(format!("环境 {} 未连接", env_id)));
    }

    let proxy_payload = match proxy {
        Some(proxy) => serde_json::to_vec(&proxy)
            .map_err(|error| RuntimeError::Serialization(error.to_string()))?,
        None => b"null".to_vec(),
    };

    manager
        .send_event(&env_id, Topic::ProxySet, proxy_payload)
        .await?;
    let _ = events.emit(
        "environment.proxy_refreshed",
        &serde_json::json!({ "env_uuid": env_id }),
    );
    Ok(())
}

pub async fn set_window_bounds(request: WindowBoundsRequest, events: EventPublisher) -> Result<()> {
    let env_id = request.env_uuid.trim().to_string();
    let manager = eventbus_manager();

    if !manager.is_connected(&env_id).await {
        return Err(RuntimeError::Internal(format!("环境 {} 未连接", env_id)));
    }

    let payload =
        encode_window_bounds_payload(request.x, request.y, request.width, request.height)?;
    let message = Message::event(Topic::WindowSetBounds, payload);
    manager.send(&env_id, &message).await?;

    let _ = events.emit(
        "environment.window_bounds_updated",
        &serde_json::json!({
            "env_uuid": env_id,
            "x": request.x,
            "y": request.y,
            "width": request.width,
            "height": request.height,
        }),
    );
    Ok(())
}

pub async fn get_connected_environments() -> Result<Vec<String>> {
    if let Some(manager) = get_eventbus_manager() {
        Ok(manager.connected_envs().await)
    } else {
        Ok(vec![])
    }
}

pub async fn get_cdp_endpoint(
    env_uuid: String,
    cdp_endpoint_manager: Arc<CdpEndpointManager>,
) -> Result<Option<CdpEndpointResponse>> {
    let env_id = env_uuid.trim().to_string();
    Ok(cdp_endpoint_manager
        .get_endpoint(&env_id)
        .await
        .map(|endpoint| CdpEndpointResponse {
            env_uuid: endpoint.env_uuid,
            host: endpoint.host,
            port: endpoint.port,
            version_url: endpoint.version_url,
            list_url: endpoint.list_url,
            browser_ws_url: endpoint.browser_ws_url,
        }))
}

pub async fn batch_launch_environments(
    requests: Vec<EnvironmentStartRequest>,
    cdp_endpoint_manager: Arc<CdpEndpointManager>,
    job_manager: Arc<JobManager>,
    status_manager: Arc<EnvironmentStatusManager>,
    events: EventPublisher,
) -> Result<Vec<BatchLaunchResult>> {
    let tasks: Vec<_> = requests
        .into_iter()
        .enumerate()
        .map(|(index, request)| {
            let env_uuid = request.env_uuid.clone();
            let cdp_endpoint_manager = cdp_endpoint_manager.clone();
            let job_manager = job_manager.clone();
            let status_manager = status_manager.clone();
            let events = events.clone();

            tokio::spawn(async move {
                let delay = (index as u64) * 50 + (rand::random::<u64>() % 200);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

                let result = launch_browser(
                    request,
                    cdp_endpoint_manager,
                    job_manager,
                    status_manager,
                    events,
                )
                .await;

                BatchLaunchResult {
                    env_uuid,
                    success: result.is_ok(),
                    error: result.err().map(|error| error.to_string()),
                }
            })
        })
        .collect();

    let mut results = Vec::new();
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    Ok(results)
}

fn encode_window_bounds_payload(x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u8>> {
    if width <= 0 || height <= 0 {
        return Err(RuntimeError::Internal("窗口宽高必须大于 0".into()));
    }

    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&x.to_le_bytes());
    payload.extend_from_slice(&y.to_le_bytes());
    payload.extend_from_slice(&width.to_le_bytes());
    payload.extend_from_slice(&height.to_le_bytes());
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::encode_window_bounds_payload;

    #[test]
    fn window_bounds_payload_is_little_endian_i32_sequence() {
        let payload = encode_window_bounds_payload(10, 20, 1280, 720).unwrap();

        assert_eq!(payload.len(), 16);
        assert_eq!(i32::from_le_bytes(payload[0..4].try_into().unwrap()), 10);
        assert_eq!(i32::from_le_bytes(payload[4..8].try_into().unwrap()), 20);
        assert_eq!(i32::from_le_bytes(payload[8..12].try_into().unwrap()), 1280);
        assert_eq!(i32::from_le_bytes(payload[12..16].try_into().unwrap()), 720);
    }

    #[test]
    fn window_bounds_payload_rejects_non_positive_size() {
        assert!(encode_window_bounds_payload(0, 0, 0, 720).is_err());
        assert!(encode_window_bounds_payload(0, 0, 1280, -1).is_err());
    }
}

pub async fn batch_stop_environments(
    env_uuids: Vec<String>,
    cdp_endpoint_manager: Arc<CdpEndpointManager>,
    job_manager: Arc<JobManager>,
    status_manager: Arc<EnvironmentStatusManager>,
    events: EventPublisher,
) -> Result<Vec<BatchLaunchResult>> {
    let tasks: Vec<_> = env_uuids
        .into_iter()
        .map(|env_uuid| {
            let cdp_endpoint_manager = cdp_endpoint_manager.clone();
            let job_manager = job_manager.clone();
            let status_manager = status_manager.clone();
            let events = events.clone();
            tokio::spawn(async move {
                let result = stop_environment(
                    env_uuid.clone(),
                    cdp_endpoint_manager,
                    job_manager,
                    status_manager,
                    events,
                )
                .await;

                BatchLaunchResult {
                    env_uuid,
                    success: result.is_ok(),
                    error: result.err().map(|error| error.to_string()),
                }
            })
        })
        .collect();

    let mut results = Vec::new();
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    Ok(results)
}
