//! The macOS services and panel, and the composition root that wires them.

pub mod panel;
pub mod services;

use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use agent_notify_core::paths::{SocketLocation, log_directory, socket_location, state_directory};
use agent_notify_core::status::DaemonFacts;
use agent_notify_daemon::actor::{Daemon, DaemonConfig};
use agent_notify_daemon::dnd_store::DndStore;
use agent_notify_daemon::environment::DaemonEnvironment;
use agent_notify_daemon::presence::IoregPresence;
use agent_notify_daemon::services::{Services, system_clock};
use agent_notify_daemon::socket::{BindOutcome, bind};
use agent_notify_daemon::tmux::Tmux;
use agent_notify_daemon::tools::ToolPaths;
use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
use tauri::{AppHandle, Manager, RunEvent};
use tokio::runtime::Handle;
use tokio::signal::unix::{SignalKind, signal};

use crate::shell::{self, Shell};
use crate::{STAMP, logging};

/// Runs the daemon until the app exits.
pub fn run() -> ExitCode {
    let environment = DaemonEnvironment::read();
    let inputs = environment.path_inputs();
    let level = logging::level_from(std::env::var("AGENT_NOTIFY_LOG").ok().as_deref());
    let log_guard = match log_directory(&inputs).map(|directory| logging::init(&directory, level)) {
        Some(Ok(guard)) => Some(guard),
        Some(Err(error)) => {
            eprintln!("agent-notifyd: logging did not start: {error}");
            None
        }
        None => None,
    };
    logging::install_panic_hook();
    let SocketLocation::At(socket) = socket_location(&inputs) else {
        tracing::error!("no socket path on this machine");
        return ExitCode::from(1);
    };
    let listener = match bind(&socket) {
        Ok(BindOutcome::Bound(listener)) => listener,
        Ok(BindOutcome::AlreadyRunning) => {
            tracing::info!("another agent-notifyd runs; exiting");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            tracing::error!(%error, socket = %socket.display(), "the socket did not bind");
            return ExitCode::from(1);
        }
    };
    let Some(state) = state_directory(&inputs) else {
        tracing::error!("no state directory on this machine");
        return ExitCode::from(1);
    };
    let tools = ToolPaths::discover(&environment);
    tracing::info!(stamp = STAMP, ?tools, socket = %socket.display(), "starting");
    let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("agent-notifyd").enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "the runtime did not start");
            return ExitCode::from(1);
        }
    };
    let _activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
        &NSString::from_str("agent-notifyd delivers agent notifications"),
    );
    let daemon_slot: Arc<Mutex<Option<Daemon>>> = Arc::new(Mutex::new(None));
    let setup_slot = Arc::clone(&daemon_slot);
    let runtime_handle = runtime.handle().clone();
    let clock = system_clock();
    let facts = DaemonFacts { stamp: STAMP.to_string(), started_ms: clock(), tools: tools.statuses() };
    let built = tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![shell::bootstrap, shell::ui_message])
        .setup(move |app| {
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let services = Arc::new(Services {
                tmux: tools.tmux.clone().map(Tmux::new),
                raiser: Arc::new(services::MacRaiser::new(handle.clone(), tools.aerospace.clone())),
                presenter: Arc::new(panel::PanelPresenter::new(handle.clone())),
                frontmost: Arc::new(services::MacFrontmost::new(handle.clone())),
                presence: Arc::new(IoregPresence),
                sound: Arc::new(services::AfplaySound),
                dnd_store: DndStore::new(state.join("dnd.json")),
                clock,
                facts,
            });
            let daemon = {
                let _entered = runtime_handle.enter();
                Daemon::start(services, listener, DaemonConfig::default())
            };
            exit_when_a_part_ends(&runtime_handle, &daemon, handle.clone());
            exit_on_signals(&runtime_handle, &handle);
            panel::build(&handle, daemon.handle()).inspect_err(|error| tracing::error!(%error, "the panel did not build"))?;
            app.manage(Shell { daemon: daemon.handle(), native_hover: true });
            if let Ok(mut slot) = setup_slot.lock() {
                *slot = Some(daemon);
            }
            Ok(())
        })
        .build(tauri::generate_context!());
    let app = match built {
        Ok(app) => app,
        Err(error) => {
            tracing::error!(%error, "tauri did not start");
            return ExitCode::from(1);
        }
    };
    let exit_handle = runtime.handle().clone();
    let mut log_guard = log_guard;
    app.run(move |_, event| {
        if let RunEvent::Exit = event {
            if let Some(daemon) = daemon_slot.lock().ok().and_then(|mut slot| slot.take()) {
                exit_handle.block_on(daemon.stop());
                tracing::info!("stopped");
            }
            drop(log_guard.take());
        }
    });
    ExitCode::SUCCESS
}

/// Exits with 1 when the actor or the socket server ends on its own, so
/// launchd restarts the daemon (`KeepAlive` with `SuccessfulExit` false).
fn exit_when_a_part_ends(runtime: &Handle, daemon: &Daemon, app: AppHandle) {
    let ended = daemon.ended_early();
    runtime.spawn(async move {
        let part = ended.await;
        tracing::error!(?part, "a daemon task ended while the app ran; exiting so launchd restarts it");
        app.exit(1);
    });
}

/// Turns SIGTERM (launchd) and SIGINT (Ctrl-C) into a clean app exit, which
/// stops the daemon in `RunEvent::Exit`. Without this, either signal ends
/// the process at once and leaves the socket file behind.
fn exit_on_signals(runtime: &Handle, app: &AppHandle) {
    for (kind, name) in [(SignalKind::terminate(), "SIGTERM"), (SignalKind::interrupt(), "SIGINT")] {
        let app = app.clone();
        runtime.spawn(async move {
            let mut received = match signal(kind) {
                Ok(received) => received,
                Err(error) => {
                    tracing::error!(%error, signal = name, "the signal handler did not install; this signal ends the app without a clean stop");
                    return;
                }
            };
            if received.recv().await.is_some() {
                tracing::info!(signal = name, "stopping on a signal");
                app.exit(0);
            }
        });
    }
}
