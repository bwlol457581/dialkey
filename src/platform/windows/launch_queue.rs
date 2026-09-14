//! Fire-and-forget slot launch on a dedicated STA thread.
//!
//! The UI / hook threads must not wait on `ShellExecute` or `EnumWindows`.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tracing::{debug, error, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_QUIT,
};

use crate::core::launch::{launch_slot, open_slot_workdir};
use crate::core::slots::{Slot, SlotRegistry};

use super::messages::WM_DK_LAUNCH_FAILED;

struct LaunchJob {
    slot: Slot,
    exe_dir: PathBuf,
    registry: SlotRegistry,
    folder_only: bool,
    notify_host: isize,
}

static TX: Mutex<Option<Sender<LaunchJob>>> = Mutex::new(None);

/// Keep this value alive for the process lifetime. Dropping it stops the worker.
pub struct LaunchWorker {
    join: Option<JoinHandle<()>>,
}

impl LaunchWorker {
    pub fn start() -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel::<LaunchJob>();
        if let Ok(mut g) = TX.lock() {
            *g = Some(tx);
        }
        let join = thread::Builder::new()
            .name("dialkey-launch".into())
            .spawn(move || worker_main(rx))
            .map_err(|e| anyhow::anyhow!("spawn launch worker: {e}"))?;
        info!("launch worker started (STA)");
        Ok(Self { join: Some(join) })
    }
}

impl Drop for LaunchWorker {
    fn drop(&mut self) {
        if let Ok(mut g) = TX.lock() {
            *g = None;
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn queue(
    slot: Slot,
    exe_dir: PathBuf,
    registry: SlotRegistry,
    folder_only: bool,
    notify_host: HWND,
) -> anyhow::Result<()> {
    let g = TX
        .lock()
        .map_err(|_| anyhow::anyhow!("launch worker lock poisoned"))?;
    let tx = g
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("launch worker not started"))?;
    tx.send(LaunchJob {
        slot,
        exe_dir,
        registry,
        folder_only,
        notify_host: notify_host.0 as isize,
    })
    .map_err(|_| anyhow::anyhow!("launch worker disconnected"))?;
    Ok(())
}

fn worker_main(rx: mpsc::Receiver<LaunchJob>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(job) => run_job(job),
            Err(RecvTimeoutError::Timeout) => pump_thread_messages(),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    unsafe {
        CoUninitialize();
    }
}

fn pump_thread_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                return;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn run_job(job: LaunchJob) {
    let id = job.slot.id.clone();
    let kind = if job.folder_only {
        "open_workdir"
    } else {
        "launch"
    };
    info!(slot_id = %id, folder_only = job.folder_only, "launch worker running");
    let started = Instant::now();
    let result = if job.folder_only {
        open_slot_workdir(&job.slot, &job.exe_dir, &job.registry)
    } else {
        launch_slot(&job.slot, &job.exe_dir, &job.registry)
    };
    let launch_ms = started.elapsed().as_millis();
    let ok = result.is_ok();
    if let Err(e) = result {
        error!(slot_id = %id, error = %e, "launch worker failed");
        if job.notify_host != 0 {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    HWND(job.notify_host as *mut _),
                    WM_DK_LAUNCH_FAILED,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }
    if launch_ms >= 250 {
        warn!(kind, slot_id = %id, launch_ms, ok, "launch worker slow");
    } else {
        debug!(kind, slot_id = %id, launch_ms, ok, "launch worker finished");
    }
}
