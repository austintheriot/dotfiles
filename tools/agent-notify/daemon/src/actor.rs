//! The daemon's tasks and threads.
//!
//! - The socket task decodes lines into two bounded queues: one for agent
//!   and control requests, one for queries. A full request queue refuses the
//!   request with no reply, so its client falls back. A query still gets in.
//! - The query task passes every query to the actor at once.
//! - The request pipeline reads focus facts on a blocking thread for a
//!   request that can show a card, then passes the request on. Requests keep
//!   their order among themselves. A query can overtake a request, so it
//!   answers with the state as of when the actor reads it.
//! - The actor task owns `DaemonState` and the decision log. Nothing else
//!   reads or writes them. It pushes a view to the presenter only when the
//!   cards, the finished row or the DND flag change.
//! - The tick task drives expiry and the pane snapshots.
//! - The effects worker thread runs focus plans, tmux writes, DND saves and
//!   pane snapshots in order, and sends results back as `DaemonInput`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use agent_notify_core::state::{DaemonState, Dnd};
use agent_notify_core::status::{Decision, MAX_DECISIONS, control_kind, decision, event_kind, status_report};
use agent_notify_core::update::{DaemonInput, Effect, FocusSnapshot, FocusTarget, PaneSnapshot, PaneState, UiIntent, may_offer_card, update};
use agent_notify_core::view::{CardView, FinishedEntry, PanelViewModel, view};
use agent_notify_core::wire::{Query, Request};
use agent_notify_core::{CardId, PaneId, PaneKey, ServerId, SessionId, SocketPath};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::focus_run::run_focus;
use crate::services::Services;
use crate::socket::{Intake, Listener, PendingQuery, ServeOptions, serve};
use crate::tmux::TmuxError;

/// How long [`Daemon::stop`] waits, in all, for the queued requests and the
/// actor.
const STOP_LIMIT: Duration = Duration::from_secs(3);

/// How long [`Daemon::stop`] then waits for the effects worker. Opening the
/// terminal is bounded at 3 s and every other child process at 1 s, so the
/// worker gets more than the 3 s that one terminal open can take. With
/// [`STOP_LIMIT`] the total stays well under launchd's 20 s exit timeout.
const WORKER_STOP_LIMIT: Duration = Duration::from_secs(4);

/// Where the actor task is in its life, for [`Daemon::actor_ended`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorLife {
    Running,
    Stopping,
    EndedEarly,
}

/// Marks the actor as ended early when its task ends while still running.
/// It is dropped on every end of the task, a panic included.
struct EndAlarm(Arc<watch::Sender<ActorLife>>);

impl Drop for EndAlarm {
    fn drop(&mut self) {
        self.0.send_if_modified(|state| {
            let early = *state == ActorLife::Running;
            if early {
                *state = ActorLife::EndedEarly;
            }
            early
        });
    }
}

/// Timing and queue sizes.
#[derive(Debug, Clone, Copy)]
pub struct DaemonConfig {
    /// The timer that drives expiry.
    pub tick: Duration,
    /// How often to read pane tags from each known tmux socket.
    pub snapshot_every: Duration,
    /// Socket limits.
    pub serve: ServeOptions,
    /// Requests that may wait for the request pipeline, and queries that may
    /// wait for the actor.
    pub intake_capacity: usize,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        DaemonConfig { tick: Duration::from_millis(250), snapshot_every: Duration::from_secs(60), serve: ServeOptions::default(), intake_capacity: 1024 }
    }
}

// One value per queue hop, as in the core's `DaemonInput`. Boxing would change every match for no gain.
#[allow(clippy::large_enum_variant)]
enum ActorMessage {
    Input(DaemonInput),
    Query(PendingQuery),
    ViewModel(oneshot::Sender<PanelViewModel>),
    Shutdown,
}

enum Job {
    Focus { card: CardId, target: FocusTarget },
    PaneState { pane: PaneKey, state: PaneState },
    SaveDnd(Dnd),
    Snapshot { socket: SocketPath, known: Vec<ServerId> },
}

struct Worker {
    thread: std::thread::JoinHandle<()>,
    finished: oneshot::Receiver<()>,
}

/// A running daemon. Only `main` holds it; callbacks hold a [`DaemonHandle`].
pub struct Daemon {
    actor: mpsc::UnboundedSender<ActorMessage>,
    serving: JoinHandle<()>,
    ticking: JoinHandle<()>,
    forwarding_queries: JoinHandle<()>,
    pipeline: JoinHandle<()>,
    acting: JoinHandle<()>,
    life: Arc<watch::Sender<ActorLife>>,
    worker: Option<Worker>,
}

impl Daemon {
    /// Starts every task and the effects worker. DND comes from the store,
    /// read here once before any task runs.
    ///
    /// # Panics
    ///
    /// Outside a Tokio runtime.
    #[must_use]
    pub fn start(services: Arc<Services>, listener: Listener, config: DaemonConfig) -> Daemon {
        let dnd = services.dnd_store.load();
        let (actor_sender, actor_receiver) = mpsc::unbounded_channel();
        let (request_sender, request_receiver) = mpsc::channel(config.intake_capacity);
        let (query_sender, query_receiver) = mpsc::channel(config.intake_capacity);
        let (job_sender, job_receiver) = std::sync::mpsc::channel();
        let worker = start_worker(Arc::clone(&services), job_receiver, actor_sender.clone());
        let stamp = services.facts.stamp.clone();
        let intake = Intake { requests: request_sender, queries: query_sender };
        let life = Arc::new(watch::Sender::new(ActorLife::Running));
        let alarm = EndAlarm(Arc::clone(&life));
        Daemon {
            serving: tokio::spawn(async move {
                if let Err(error) = serve(listener, stamp, intake, config.serve).await {
                    tracing::error!(%error, "the socket server stopped");
                }
            }),
            ticking: tokio::spawn(run_ticks(actor_sender.clone(), config.tick)),
            forwarding_queries: tokio::spawn(run_queries(query_receiver, actor_sender.clone())),
            pipeline: tokio::spawn(run_requests(Arc::clone(&services), request_receiver, actor_sender.clone())),
            acting: tokio::spawn(async move {
                let _alarm = alarm;
                run_actor(services, actor_receiver, job_sender, config, dnd).await;
            }),
            actor: actor_sender,
            life,
            worker,
        }
    }

    /// A cheap handle for callbacks that outlive a borrow of the daemon.
    #[must_use]
    pub fn handle(&self) -> DaemonHandle {
        DaemonHandle { actor: self.actor.clone() }
    }

    /// Passes a UI intent to the core.
    pub fn send_ui(&self, intent: UiIntent) {
        self.handle().send_ui(intent);
    }

    /// The current view model, for the page's bootstrap.
    pub async fn view_model(&self) -> Option<PanelViewModel> {
        self.handle().view_model().await
    }

    /// Resolves when the actor task ends while the daemon runs: a panic, or
    /// any end that [`Daemon::stop`] did not start. It never resolves after
    /// `stop` begins. The future owns what it needs, so it can outlive `self`.
    pub fn actor_ended(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut life = self.life.subscribe();
        async move {
            if life.wait_for(|state| *state == ActorLife::EndedEarly).await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    }

    /// Stops the daemon without losing acknowledged work, within 7 s in all.
    ///
    /// 1. The socket stops, which removes the socket file and closes both
    ///    intake queues. The timer stops.
    /// 2. The query task and the request pipeline finish what is queued,
    ///    focus reads included.
    /// 3. The actor applies every message before the shutdown message, then
    ///    ends. That drops the job queue.
    /// 4. The effects worker finishes its queued jobs.
    ///
    /// Steps 2 and 3 share [`STOP_LIMIT`]; step 4 then gets
    /// [`WORKER_STOP_LIMIT`]. A step still busy at its limit is logged and
    /// left behind: a task is aborted, the worker thread ends on its own.
    pub async fn stop(self) {
        self.life.send_replace(ActorLife::Stopping);
        let deadline = Instant::now() + STOP_LIMIT;
        abort_and_wait(self.serving).await;
        abort_and_wait(self.ticking).await;
        finish_by(self.forwarding_queries, deadline, "the query task").await;
        finish_by(self.pipeline, deadline, "the request pipeline").await;
        if self.actor.send(ActorMessage::Shutdown).is_err() {
            tracing::error!("the actor stopped before the daemon did");
        }
        drop(self.actor);
        finish_by(self.acting, deadline, "the actor").await;
        let Some(worker) = self.worker else { return };
        if tokio::time::timeout(WORKER_STOP_LIMIT, worker.finished).await.is_err() {
            tracing::error!(limit_ms = WORKER_STOP_LIMIT.as_millis(), "the effects worker did not stop in time; left running");
            return;
        }
        if let Ok(Err(_)) = tokio::task::spawn_blocking(move || worker.thread.join()).await {
            tracing::error!("the effects worker panicked");
        }
    }
}

async fn abort_and_wait(task: JoinHandle<()>) {
    task.abort();
    if let Err(error) = task.await
        && !error.is_cancelled()
    {
        tracing::error!(%error, "a daemon task failed");
    }
}

async fn finish_by(mut task: JoinHandle<()>, deadline: Instant, name: &'static str) {
    match tokio::time::timeout_at(deadline, &mut task).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::error!(%error, task = name, "a daemon task failed"),
        Err(_) => {
            tracing::error!(task = name, limit_ms = STOP_LIMIT.as_millis(), "queued work did not finish in time; aborted");
            abort_and_wait(task).await;
        }
    }
}

/// Sends UI intents and reads the view model. Clone it into callbacks.
#[derive(Clone)]
pub struct DaemonHandle {
    actor: mpsc::UnboundedSender<ActorMessage>,
}

impl DaemonHandle {
    /// Passes a UI intent to the core.
    pub fn send_ui(&self, intent: UiIntent) {
        if self.actor.send(ActorMessage::Input(DaemonInput::Ui(intent))).is_err() {
            tracing::warn!("a UI intent arrived after the actor stopped");
        }
    }

    /// The current view model, or `None` once the actor stopped.
    pub async fn view_model(&self) -> Option<PanelViewModel> {
        let (sender, receiver) = oneshot::channel();
        self.actor.send(ActorMessage::ViewModel(sender)).ok()?;
        receiver.await.ok()
    }
}

fn start_worker(services: Arc<Services>, jobs: std::sync::mpsc::Receiver<Job>, results: mpsc::UnboundedSender<ActorMessage>) -> Option<Worker> {
    let (finished_sender, finished) = oneshot::channel();
    let spawned = std::thread::Builder::new().name("agent-notify-effects".to_string()).spawn(move || {
        run_worker(&services, &jobs, &results);
        let _ = finished_sender.send(());
    });
    match spawned {
        Ok(thread) => Some(Worker { thread, finished }),
        Err(error) => {
            tracing::error!(%error, "the effects worker did not start; effects are dropped");
            None
        }
    }
}

async fn run_ticks(actor: mpsc::UnboundedSender<ActorMessage>, every: Duration) {
    let mut interval = tokio::time::interval(every);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if actor.send(ActorMessage::Input(DaemonInput::Tick)).is_err() {
            return;
        }
    }
}

async fn run_queries(mut queries: mpsc::Receiver<PendingQuery>, actor: mpsc::UnboundedSender<ActorMessage>) {
    while let Some(query) = queries.recv().await {
        if actor.send(ActorMessage::Query(query)).is_err() {
            return;
        }
    }
}

async fn run_requests(services: Arc<Services>, mut requests: mpsc::Receiver<Request>, actor: mpsc::UnboundedSender<ActorMessage>) {
    while let Some(request) = requests.recv().await {
        let focus = if may_offer_card(&request) { gather_off_the_runtime(&services, &request).await } else { FocusSnapshot::unknown() };
        if actor.send(ActorMessage::Input(DaemonInput::Received { request, focus })).is_err() {
            return;
        }
    }
}

async fn gather_off_the_runtime(services: &Arc<Services>, request: &Request) -> FocusSnapshot {
    let reading = Arc::clone(services);
    let copy = request.clone();
    tokio::task::spawn_blocking(move || gather_focus(&reading, &copy)).await.unwrap_or_else(|error| {
        tracing::error!(%error, "focus gathering failed; every fact reads as unknown");
        FocusSnapshot::unknown()
    })
}

/// Reads the focus facts a request needs: presence, the frontmost app and
/// the active tmux pane, only for an agent event that can show a card.
/// Every other request gets [`FocusSnapshot::unknown`] without any read.
/// The reads block, so call it on a blocking thread.
#[must_use]
pub fn gather_focus(services: &Services, request: &Request) -> FocusSnapshot {
    let Request::Agent { origin, .. } = request else { return FocusSnapshot::unknown() };
    if !may_offer_card(request) {
        return FocusSnapshot::unknown();
    }
    let presence = services.presence.presence();
    let active_pane = match (&services.tmux, &origin.pane) {
        (Some(tmux), Some(pane)) => tmux.active_pane(&pane.server).unwrap_or_else(|error| {
            tracing::debug!(%error, "no active pane");
            None
        }),
        _ => None,
    };
    FocusSnapshot {
        frontmost: services.frontmost.frontmost(),
        active_pane,
        idle_seconds: presence.idle_seconds,
        screen_locked: presence.screen_locked,
        screen_shared: None,
    }
}

/// What the log says about the request behind a decision. Never any text
/// the agent or the user wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestContext {
    session: Option<SessionId>,
    pane: Option<PaneId>,
    kind: &'static str,
}

impl RequestContext {
    fn of(request: &Request) -> Option<RequestContext> {
        match request {
            Request::Agent { origin, event, .. } => Some(RequestContext {
                session: origin.session.clone(),
                pane: origin.pane.as_ref().map(|pane| pane.pane.clone()),
                kind: event_kind(event),
            }),
            Request::Control { control, .. } => Some(RequestContext { session: None, pane: None, kind: control_kind(control) }),
            Request::Query { .. } | Request::Unknown => None,
        }
    }
}

struct Shown {
    cards: Vec<CardView>,
    finished_row: Vec<FinishedEntry>,
    dnd: bool,
}

impl Shown {
    fn of(model: &PanelViewModel) -> Shown {
        Shown { cards: model.cards.clone(), finished_row: model.finished_row.clone(), dnd: model.dnd }
    }

    fn differs(&self, model: &PanelViewModel) -> bool {
        self.cards != model.cards || self.finished_row != model.finished_row || self.dnd != model.dnd
    }
}

// The snapshot pace uses the monotonic clock: a wall clock that steps back would stop snapshots until it caught up.
async fn run_actor(services: Arc<Services>, mut inbox: mpsc::UnboundedReceiver<ActorMessage>, jobs: std::sync::mpsc::Sender<Job>, config: DaemonConfig, dnd: Dnd) {
    let mut state = DaemonState { dnd, ..DaemonState::default() };
    let mut decisions: VecDeque<Decision> = VecDeque::with_capacity(MAX_DECISIONS);
    let mut shown: Option<Shown> = None;
    let mut last_snapshot = Instant::now();
    while let Some(message) = inbox.recv().await {
        let now = (services.clock)();
        match message {
            ActorMessage::Input(input) => {
                let received = match &input {
                    DaemonInput::Received { request, .. } => Some(request.clone()),
                    _ => None,
                };
                let is_tick = matches!(input, DaemonInput::Tick);
                let effects = update(&mut state, input, now);
                if let Some(entry) = received.as_ref().and_then(|request| decision(request, &effects, now)) {
                    if decisions.len() == MAX_DECISIONS {
                        decisions.pop_front();
                    }
                    decisions.push_back(entry);
                }
                apply(&services, &jobs, effects, received.as_ref().and_then(RequestContext::of).as_ref());
                if is_tick && services.tmux.is_some() && last_snapshot.elapsed() >= config.snapshot_every {
                    last_snapshot = Instant::now();
                    for (socket, known) in servers_by_socket(&state) {
                        send_job(&jobs, Job::Snapshot { socket, known });
                    }
                }
                let model = view(&state, now);
                if shown.as_ref().is_none_or(|previous| previous.differs(&model)) {
                    services.presenter.present(&model);
                    shown = Some(Shown::of(&model));
                }
            }
            ActorMessage::Query(PendingQuery { query, answer }) => {
                let data = match query {
                    Query::Status => {
                        let log: Vec<Decision> = decisions.iter().cloned().collect();
                        serde_json::to_value(status_report(&state, now, &services.facts, &log))
                    }
                    Query::ViewModel => serde_json::to_value(view(&state, now)),
                    Query::Unknown => Ok(serde_json::Value::Null),
                };
                let data = data.unwrap_or_else(|error| {
                    tracing::error!(%error, "a query answer did not encode");
                    serde_json::Value::Null
                });
                if answer.send(data).is_err() {
                    tracing::debug!("a query's client left before its answer");
                }
            }
            ActorMessage::ViewModel(answer) => {
                if answer.send(view(&state, now)).is_err() {
                    tracing::debug!("a view model request left before its answer");
                }
            }
            ActorMessage::Shutdown => return,
        }
    }
}

/// The distinct tmux sockets of the records, each with the servers the
/// records name on it.
fn servers_by_socket(state: &DaemonState) -> BTreeMap<SocketPath, Vec<ServerId>> {
    let mut sockets: BTreeMap<SocketPath, Vec<ServerId>> = BTreeMap::new();
    for server in state.sessions.values().filter_map(|record| record.pane.as_ref().map(|pane| &pane.server)) {
        let known = sockets.entry(server.socket_path.clone()).or_default();
        if !known.contains(server) {
            known.push(server.clone());
        }
    }
    sockets
}

fn apply(services: &Services, jobs: &std::sync::mpsc::Sender<Job>, effects: Vec<Effect>, context: Option<&RequestContext>) {
    for effect in effects {
        match effect {
            Effect::RunFocusPlan { card, target } => send_job(jobs, Job::Focus { card, target }),
            Effect::PlaySound => services.sound.play(),
            Effect::SetPaneState { pane, state } => send_job(jobs, Job::PaneState { pane, state }),
            Effect::PersistDnd(dnd) => send_job(jobs, Job::SaveDnd(dnd)),
            Effect::Suppressed { event_id, reason } => tracing::info!(
                %event_id,
                session_id = context.and_then(|context| context.session.as_ref()).map_or("-", SessionId::as_str),
                pane = context.and_then(|context| context.pane.as_ref()).map_or("-", PaneId::as_str),
                kind = context.map_or("-", |context| context.kind),
                ?reason,
                "suppressed"
            ),
        }
    }
}

fn send_job(jobs: &std::sync::mpsc::Sender<Job>, job: Job) {
    if jobs.send(job).is_err() {
        tracing::error!("the effects worker is gone; an effect was dropped");
    }
}

fn run_worker(services: &Services, jobs: &std::sync::mpsc::Receiver<Job>, results: &mpsc::UnboundedSender<ActorMessage>) {
    let report = |input: DaemonInput| {
        if results.send(ActorMessage::Input(input)).is_err() {
            tracing::debug!("a worker result arrived after the actor stopped");
        }
    };
    for job in jobs {
        match job {
            Job::Focus { card, target } => {
                let outcome = run_focus(services.tmux.as_ref(), services.raiser.as_ref(), &target);
                report(DaemonInput::FocusOutcome { card, outcome });
            }
            Job::PaneState { pane, state } => {
                if let Some(tmux) = &services.tmux
                    && let Err(error) = tmux.set_pane_state(&pane, state)
                {
                    tracing::warn!(%error, pane = pane.pane.as_str(), ?state, "pane state not written; the next pane snapshot writes it again");
                }
            }
            Job::SaveDnd(dnd) => {
                if let Err(error) = services.dnd_store.save(&dnd) {
                    tracing::error!(%error, "DND not saved; it resets on restart");
                }
            }
            Job::Snapshot { socket, known } => match services.tmux.as_ref().map(|tmux| tmux.pane_snapshot(&socket)) {
                Some(Ok(snapshot)) => report(DaemonInput::PaneSnapshot(snapshot)),
                Some(Err(TmuxError::ServerGone)) => {
                    for server in known {
                        report(DaemonInput::PaneSnapshot(PaneSnapshot { server, panes: Vec::new() }));
                    }
                }
                Some(Err(error)) => tracing::warn!(%error, socket = %socket.as_path().display(), "pane snapshot failed; sessions are kept"),
                None => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use agent_notify_core::state::SessionRecord;
    use agent_notify_core::wire::{AgentEvent, AgentKind, Control, Origin, WIRE_VERSION};
    use agent_notify_core::{EpochMs, EventId};

    use super::*;

    fn agent(event: AgentEvent, pane: Option<PaneKey>) -> Request {
        Request::Agent {
            v: WIRE_VERSION,
            event_id: EventId::from_parts(EpochMs(1), [0; 10]),
            sent_at_ms: EpochMs(1),
            origin: Origin {
                agent: AgentKind::ClaudeCode,
                session: Some(SessionId::parse("session-a").expect("id")),
                prompt: None,
                pane,
                window: None,
                repo: None,
                session_title: None,
                cwd: PathBuf::from("/tmp"),
            },
            event,
        }
    }

    fn pane_on(socket: &str, pid: u32, pane: &str) -> PaneKey {
        let server = ServerId { socket_path: SocketPath::parse(std::path::Path::new(socket)).expect("absolute"), pid, start_time: 1 };
        PaneKey { server, pane: PaneId::parse(pane).expect("id") }
    }

    #[test]
    fn the_log_context_names_the_session_pane_and_kind_only() {
        let context = RequestContext::of(&agent(AgentEvent::TurnStarted, Some(pane_on("/tmp/s", 1, "%3")))).expect("an agent request");
        assert_eq!(context, RequestContext { session: Some(SessionId::parse("session-a").expect("id")), pane: Some(PaneId::parse("%3").expect("id")), kind: "turn_started" });
        let control = Request::Control { v: WIRE_VERSION, event_id: EventId::from_parts(EpochMs(1), [0; 10]), sent_at_ms: EpochMs(1), control: Control::Jump };
        assert_eq!(RequestContext::of(&control).map(|context| context.kind), Some("jump"));
    }

    #[test]
    fn snapshots_go_once_per_socket_with_every_server_known_on_it() {
        let panes = [pane_on("/tmp/a", 1, "%1"), pane_on("/tmp/a", 1, "%2"), pane_on("/tmp/a", 2, "%1"), pane_on("/tmp/b", 7, "%1")];
        let sessions = panes
            .into_iter()
            .enumerate()
            .map(|(number, pane)| {
                let Request::Agent { mut origin, .. } = agent(AgentEvent::TurnStarted, Some(pane)) else { panic!("an agent request") };
                origin.session = Some(SessionId::parse(&format!("session-{number}")).expect("id"));
                (origin.session.clone().expect("set above"), SessionRecord::new(&origin))
            })
            .collect();
        let grouped = servers_by_socket(&DaemonState { sessions, ..DaemonState::default() });
        let counts: Vec<(String, Vec<u32>)> =
            grouped.iter().map(|(socket, known)| (socket.as_path().display().to_string(), known.iter().map(|server| server.pid).collect())).collect();
        assert_eq!(counts, vec![("/tmp/a".to_string(), vec![1, 2]), ("/tmp/b".to_string(), vec![7])]);
    }
}
