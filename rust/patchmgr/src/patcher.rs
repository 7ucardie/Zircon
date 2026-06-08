//! `zircon-patcher` — self-update utility.
//!
//! Called by the launcher when it has downloaded its own replacement binary:
//!
//!   zircon-patcher <patch_from> <patch_to>
//!
//! Waits briefly for the calling process to exit, then replaces `patch_to`
//! with `patch_from` and relaunches the updated binary.

use std::{env, path::PathBuf, process::Command, thread, time::Duration};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: zircon-patcher <patch_from> <patch_to>");
        std::process::exit(1);
    }
    let from = PathBuf::from(&args[1]);
    let to   = PathBuf::from(&args[2]);

    // Allow the calling process time to release file handles.
    thread::sleep(Duration::from_secs(2));

    // Remove old binary (ignore NotFound — already gone is fine).
    if let Err(e) = std::fs::remove_file(&to) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("Failed to remove {}: {e}", to.display());
            std::process::exit(1);
        }
    }

    // Move new binary into place.
    if let Err(e) = std::fs::rename(&from, &to) {
        eprintln!("Failed to install {}: {e}", to.display());
        std::process::exit(1);
    }

    // Relaunch.
    if let Err(e) = Command::new(&to).spawn() {
        eprintln!("Failed to launch {}: {e}", to.display());
        std::process::exit(1);
    }
}
