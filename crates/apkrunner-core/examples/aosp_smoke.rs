use std::env;
use std::path::PathBuf;

use apkrunner_core::{
    AppConfiguration, BackendKind, InputEvent, Runner, RunnerConfiguration,
    RuntimeBundleConfiguration, RuntimeEvent,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let apk_path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: cargo run -p apkrunner-core --example aosp_smoke -- <apk> <bundle-root> <sandbox-root>")?;
    let bundle_root = args
        .next()
        .map(PathBuf::from)
        .ok_or("missing bundle root")?;
    let sandbox_root = args
        .next()
        .map(PathBuf::from)
        .ok_or("missing sandbox root")?;

    let mut runner = Runner::new(RunnerConfiguration {
        backend_kind: BackendKind::Aosp,
        sandbox_root,
        runtime_bundle: RuntimeBundleConfiguration {
            bundle_root: Some(bundle_root),
            boot_timeout_ms: 180_000,
            command_timeout_ms: 60_000,
            terminate_emulator_on_stop: true,
            ..RuntimeBundleConfiguration::default()
        },
    })?;

    runner.load_apk_from_host_path(apk_path)?;
    let instance = runner.create_app_instance(AppConfiguration { package_name: None })?;
    runner.start_app(instance)?;
    runner.dispatch_input(instance, InputEvent::Tap { x: 40, y: 40 })?;

    let events = runner.poll_runtime_events();
    for event in &events {
        println!("{}", summarize_event(event));
    }

    runner.stop_app(instance)?;
    for event in runner.poll_runtime_events() {
        println!("{}", summarize_event(&event));
    }

    Ok(())
}

fn summarize_event(event: &RuntimeEvent) -> String {
    match event {
        RuntimeEvent::Log {
            level,
            tag,
            message,
            ..
        } => {
            format!("Log {:?}/{tag}: {message}", level)
        }
        RuntimeEvent::ApkLoaded { summary } => {
            format!("ApkLoaded {}", summary.package_name)
        }
        RuntimeEvent::PermissionRequest { permission } => {
            format!("PermissionRequest {}", permission.name)
        }
        RuntimeEvent::AppStarted {
            package_name,
            instance_id,
        } => {
            format!("AppStarted {package_name} {instance_id}")
        }
        RuntimeEvent::AppStopped {
            package_name,
            instance_id,
        } => {
            format!("AppStopped {package_name} {instance_id}")
        }
        RuntimeEvent::UnsupportedFeature { feature } => {
            format!("UnsupportedFeature {}", feature.feature)
        }
        RuntimeEvent::FrameReady {
            surface_size,
            frame_format,
            payload_base64,
            ..
        } => {
            format!(
                "FrameReady {:?} {}x{} payload={} bytes(base64)",
                frame_format,
                surface_size.width,
                surface_size.height,
                payload_base64.len()
            )
        }
    }
}
