use fern::{
    colors::{Color, ColoredLevelConfig},
    Dispatch,
};
use syslog::Formatter3164;

/// Creates a syslog dispatcher for sending messages to the syslog
pub fn create_syslog_dispatcher<'a>(
    colors_line: ColoredLevelConfig,
    syslog_formatter: &Formatter3164,
) -> Dispatch {
    return fern::Dispatch::new()
        .level(log::LevelFilter::Info)
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}{}\x1B[0m",
                format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                message
            ));
        })
        .chain(syslog::unix(syslog_formatter.clone()).unwrap());
}

/// Sets up regular logging
pub fn setup_log(verbose: bool) {
    let syslog_formatter = syslog::Formatter3164 {
        facility: syslog::Facility::LOG_USER,
        hostname: None,
        process: "youtubeservice".to_owned(),
        pid: 0,
    };
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::White)
        .trace(Color::BrightBlack);
    let colors_level = colors_line.clone().info(Color::Green);

    let syslog_dispatcher: Dispatch;
    if let Some(server) = std::env::var_os("SYSLOG_SERVER") {
        if let Ok(tcp) = syslog::tcp(syslog_formatter.clone(), server.into_string().unwrap()) {
            syslog_dispatcher = create_syslog_dispatcher(colors_line, &syslog_formatter).chain(tcp);
        } else {
            syslog_dispatcher = create_syslog_dispatcher(colors_line, &syslog_formatter);
        }
    } else {
        syslog_dispatcher = create_syslog_dispatcher(colors_line, &syslog_formatter);
    }

    fern::Dispatch::new()
        .chain(
            fern::Dispatch::new()
                .level(if verbose {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "{color_line}[{date}][{target}][{level}{color_line}] {message}\x1B[0m",
                        color_line = format_args!(
                            "\x1B[{}m",
                            colors_line.get_color(&record.level()).to_fg_str()
                        ),
                        date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        target = record.target(),
                        level = colors_level.color(record.level()),
                        message = message,
                    ));
                })
                .chain(std::io::stdout()),
        )
        .chain(syslog_dispatcher)
        .apply()
        .unwrap();
}