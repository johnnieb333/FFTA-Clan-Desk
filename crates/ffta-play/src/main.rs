#![cfg_attr(windows, windows_subsystem = "windows")]
use std::{env,fs,path::{Path,PathBuf},process::{Command,Stdio}};
use sha2::{Digest,Sha256};
#[cfg(windows)]use std::os::windows::process::CommandExt;
const EUROPE:&str="8fab1e390cb9d5e695bfdb86e81246892e1aa8c0cb4014db8235ff76538ea632";
fn error(message:&str){let _=rfd::MessageDialog::new().set_title("FFTA Clan Desk").set_description(message).set_level(rfd::MessageLevel::Error).show();}
fn verified(path:&Path)->bool{fs::metadata(path).map(|m|m.len()==16*1024*1024).unwrap_or(false)&&fs::read(path).map(|b|format!("{:x}",Sha256::digest(b))==EUROPE).unwrap_or(false)}
fn main(){
 let Some(exe_dir)=env::current_exe().ok().and_then(|p|p.parent().map(Path::to_path_buf))else{return error("Cannot locate the launcher folder.");};
 let recomp=exe_dir.join(if cfg!(windows){"recomp.exe"}else{"recomp"});if !recomp.is_file(){return error("recomp.exe is missing. Extract the complete release archive before launching.");}
 let data=dirs::data_local_dir().unwrap_or_else(||exe_dir.clone());let config=data.join("FFTA-Clan-Desk");let remembered=config.join("rom-path.txt");
 let arg=env::args_os().nth(1).map(PathBuf::from);let remembered_path=fs::read_to_string(&remembered).ok().map(|s|PathBuf::from(s.trim()));
 let candidates=[arg,Some(exe_dir.join("ffta.gba")),remembered_path,Some(data.join("FFTARecomp/ffta.gba"))];
 let rom=match candidates.into_iter().flatten().find(|p|verified(p)){
  Some(p)=>p,None=>{let Some(p)=rfd::FileDialog::new().set_title("Select Final Fantasy Tactics Advance (Europe)").add_filter("GBA ROM",&["gba"]).pick_file()else{return;};
   if !verified(&p){return error("This Windows beta supports the verified European FFTA ROM. Choose your own unmodified European cartridge image. The selected file was not changed.");}p}
 };
 let rom=fs::canonicalize(&rom).unwrap_or(rom);if fs::create_dir_all(&config).is_ok(){let _=fs::write(&remembered,rom.to_string_lossy().as_bytes());}
 let mut cmd=Command::new(&recomp);cmd.arg("play").arg(&rom);if exe_dir.join("gamedb.sqlite").is_file(){cmd.arg("--gamedb").arg(exe_dir.join("gamedb.sqlite"));}
 cmd.current_dir(&exe_dir).env("RECOMP_JOBS","2");
 for key in["FFTA_UI_LAYER","FFTA_MANUAL_STATES"]{cmd.env(key,env::var_os(key).unwrap_or_else(||"1".into()));}
 #[cfg(any(target_os="linux",windows))]cmd.env("FFTA_FRAME_PACING",env::var_os("FFTA_FRAME_PACING").unwrap_or_else(||"clock".into()));
 let mut paths=vec![exe_dir.join("toolchain/bin")];paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));if let Ok(path)=env::join_paths(paths){cmd.env("PATH",path);}
 let log=fs::File::create(config.join("play.log"));if let Ok(log)=log{if let Ok(other)=log.try_clone(){cmd.stdout(Stdio::from(other));}cmd.stderr(Stdio::from(log));}
 #[cfg(windows)]cmd.creation_flags(0x08000000);
 match cmd.status(){Ok(status)if status.success()=>{},Ok(status)=>error(&format!("The game exited with {status}. See the play.log file under Local AppData/FFTA-Clan-Desk for details.")),Err(e)=>error(&format!("Could not start the game: {e}"))}
}
