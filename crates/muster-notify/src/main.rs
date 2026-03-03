fn main() {
    let args: Vec<String> = std::env::args().collect();
    // muster-notify <summary> [body]
    let summary = args.get(1).map_or("Muster", |s| s.as_str());
    let body = args.get(2).map_or("", |s| s.as_str());

    let _ = mac_notification_sys::set_application("com.muster.notify");

    let _ = mac_notification_sys::Notification::new()
        .title(summary)
        .message(body)
        .send();
}
