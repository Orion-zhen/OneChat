fn main() {
    if !onechat::desktop::run_snapshot_helper_if_requested() {
        onechat::desktop::run();
    }
}
