fn main() {
    tauri_build::build();
    copy_libtorch_dlls();
}

#[cfg(target_os = "windows")]
fn copy_libtorch_dlls() {
    use std::env;
    use std::fs;

    println!("cargo:rerun-if-env-changed=LIBTORCH");
    println!("cargo:rerun-if-env-changed=TORCH_CUDA_VERSION");
    println!("cargo:rerun-if-env-changed=LINK_SKIP_LIBTORCH_DLL_COPY");

    if env::var_os("LINK_SKIP_LIBTORCH_DLL_COPY").is_some() {
        return;
    }

    let Some(profile_dir) = target_profile_dir() else {
        println!("cargo:warning=无法定位 target 输出目录，跳过 LibTorch DLL 同步");
        return;
    };
    let Some(libtorch_lib_dir) = libtorch_lib_dir(&profile_dir) else {
        println!("cargo:warning=无法定位 LibTorch lib 目录，跳过 LibTorch DLL 同步");
        return;
    };

    let entries = match fs::read_dir(&libtorch_lib_dir) {
        Ok(entries) => entries,
        Err(error) => {
            println!(
                "cargo:warning=无法读取 LibTorch lib 目录 {}: {error}",
                libtorch_lib_dir.display()
            );
            return;
        }
    };

    for entry in entries.flatten() {
        let source = entry.path();
        if source.extension().and_then(|value| value.to_str()) != Some("dll") {
            continue;
        }
        let Some(file_name) = source.file_name() else {
            continue;
        };
        let destination = profile_dir.join(file_name);
        if same_size_file(&source, &destination) {
            continue;
        }
        match sync_libtorch_dll(&source, &destination) {
            Ok(()) => {}
            Err(error) => println!(
                "cargo:warning=无法同步 LibTorch DLL {} -> {}: {error}",
                source.display(),
                destination.display()
            ),
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn copy_libtorch_dlls() {}

#[cfg(target_os = "windows")]
fn target_profile_dir() -> Option<std::path::PathBuf> {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR")?);
    out_dir.ancestors().nth(3).map(std::path::Path::to_path_buf)
}

#[cfg(target_os = "windows")]
fn libtorch_lib_dir(profile_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    env::var_os("DEP_TCH_LIBTORCH_LIB")
        .map(PathBuf::from)
        .filter(|path| path.join("torch_cuda.dll").is_file())
        .or_else(|| {
            env::var_os("LIBTORCH_LIB")
                .map(PathBuf::from)
                .filter(|path| path.join("torch_cuda.dll").is_file())
        })
        .or_else(|| {
            env::var_os("LIBTORCH")
                .map(PathBuf::from)
                .map(|path| path.join("lib"))
                .filter(|path| path.join("torch_cuda.dll").is_file())
        })
        .or_else(|| {
            let build_dir = profile_dir.join("build");
            let mut candidates = fs::read_dir(build_dir)
                .ok()?
                .flatten()
                .map(|entry| {
                    entry
                        .path()
                        .join("out")
                        .join("libtorch")
                        .join("libtorch")
                        .join("lib")
                })
                .filter(|path| path.join("torch_cuda.dll").is_file())
                .map(|path| {
                    let modified = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok();
                    (modified, path)
                })
                .collect::<Vec<_>>();
            candidates.sort_by_key(|(modified, _)| *modified);
            candidates.pop().map(|(_, path)| path)
        })
}

#[cfg(target_os = "windows")]
fn sync_libtorch_dll(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    if destination.exists() {
        let _ = std::fs::remove_file(destination);
    }

    match std::fs::hard_link(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => std::fs::copy(source, destination).map(|_| ()),
    }
}

#[cfg(target_os = "windows")]
fn same_size_file(source: &std::path::Path, destination: &std::path::Path) -> bool {
    let Ok(source_metadata) = std::fs::metadata(source) else {
        return false;
    };
    let Ok(destination_metadata) = std::fs::metadata(destination) else {
        return false;
    };

    source_metadata.len() == destination_metadata.len()
}
