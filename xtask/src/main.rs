#![forbid(unsafe_code)]

mod file_sizes;
mod package_floors;
mod recipe_policy;
mod simdoc;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    let result = match args.get(1).map(String::as_str) {
        Some("simdoc") => simdoc::run(args),
        Some("check-file-sizes") => file_sizes::run(&args),
        Some("check-package-floors") => package_floors::run(&args),
        Some("check-recipes") => recipe_policy::run(&args),
        _ => Err(format!(
            "usage: {program} simdoc [--check] | check-file-sizes | check-package-floors | check-recipes"
        )),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
