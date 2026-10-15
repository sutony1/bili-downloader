use std::{
    ops::{Deref, DerefMut},
    os::windows::io::AsRawHandle,
    process::{Child, Command, ExitStatus},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
};

pub const STOPPED: &str = "下载已停止";
pub fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(STOPPED.into())
    } else {
        Ok(())
    }
}

/// A job contains only this download's process tree (including yt-dlp's FFmpeg).
/// Closing it on cancellation also closes inherited stdout/stderr pipes.
pub struct ManagedChild {
    child: Child,
    job: Option<HANDLE>,
}
impl ManagedChild {
    pub fn spawn(command: &mut Command) -> Result<Self, String> {
        unsafe {
            let job = CreateJobObjectW(None, None).map_err(|e| e.to_string())?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if let Err(error) = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) {
                let _ = CloseHandle(job);
                return Err(error.to_string());
            }
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    let _ = CloseHandle(job);
                    return Err(error.to_string());
                }
            };
            if let Err(error) = AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = CloseHandle(job);
                return Err(format!("无法管理下载进程：{error}"));
            }
            Ok(Self {
                child,
                job: Some(job),
            })
        }
    }
    pub fn terminate(&mut self) {
        if let Some(job) = self.job.take() {
            unsafe {
                let _ = CloseHandle(job);
            }
        }
        let _ = self.child.wait();
    }
    pub fn check_cancel(&mut self, cancel: &AtomicBool) -> Result<(), String> {
        if check(cancel).is_err() {
            self.terminate();
            Err(STOPPED.into())
        } else {
            Ok(())
        }
    }
    pub fn wait_cancellable(&mut self, cancel: &AtomicBool) -> Result<ExitStatus, String> {
        loop {
            self.check_cancel(cancel)?;
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
impl Deref for ManagedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}
impl DerefMut for ManagedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            unsafe {
                let _ = CloseHandle(job);
            }
        }
    }
}

pub fn result(error: String) -> serde_json::Value {
    if error == STOPPED {
        serde_json::json!({"status":"下载已停止","ready":true,"log":"可以重新选择画质、再次下载，或清空网址下载其他视频。未完成的临时文件保留在下载文件夹。"})
    } else {
        serde_json::json!({"status":"下载失败","ready":true,"log":error})
    }
}
