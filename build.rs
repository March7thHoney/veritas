use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let build_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs();
    println!("cargo:rustc-env=VERITAS_BUILD_ID={build_id}");

    let ver = env!("CARGO_PKG_VERSION")
        .split('.')
        .map(|x| x.parse::<u64>().unwrap())
        .collect::<Vec<u64>>();
    let sem_ver = ver[0] << 48 | ver[1] << 32 | ver[1] << 16;

    if env::var("HOST").is_ok_and(|host| host.contains("windows")) {
        winres::WindowsResource::new()
            .set_version_info(winres::VersionInfo::PRODUCTVERSION, sem_ver)
            .compile()
            .unwrap();
        return;
    }

    println!("cargo:rerun-if-env-changed=LLVM_RC");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let rc_path = out_dir.join("resource.rc");
    let res_path = out_dir.join("resource.res");
    let resource = format!(
        r#"#pragma code_page(65001)
1 VERSIONINFO
FILESUBTYPE 0x0
FILEFLAGS 0x0
FILEOS 0x40004
FILEVERSION {}, {}, {}, 0
FILEFLAGSMASK 0x3f
PRODUCTVERSION {}, {}, {}, 0
FILETYPE 0x1
{{
BLOCK "StringFileInfo"
{{
BLOCK "000004b0"
{{
VALUE "FileDescription", "veritas"
VALUE "ProductName", "veritas"
VALUE "ProductVersion", "{}"
VALUE "FileVersion", "{}"
}}
}}
BLOCK "VarFileInfo" {{
VALUE "Translation", 0x0, 0x04b0
}}
}}
"#,
        ver[0],
        ver[1],
        ver[2],
        ver[0],
        ver[1],
        ver[1],
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_VERSION")
    );
    fs::write(&rc_path, resource).unwrap();
    let llvm_rc = env::var_os("LLVM_RC").unwrap_or_else(|| "llvm-rc".into());
    let status = Command::new(llvm_rc)
        .current_dir(&out_dir)
        .env_remove("RCFLAGS")
        .arg("/FOresource.res")
        .arg("resource.rc")
        .status()
        .expect("failed to run llvm-rc");
    assert!(status.success(), "llvm-rc failed to compile resources");
    println!("cargo:rustc-link-arg={}", res_path.display());
}
