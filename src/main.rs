// SPDX-License-Identifier: MIT

//! The GUI binary. Everything of substance lives in the `clocks` library, so
//! the daemon can share it.

fn main() -> cosmic::iced::Result {
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    clocks::i18n::init(&requested_languages);

    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(360.0)
            .min_height(180.0),
    );

    cosmic::app::run::<clocks::app::AppModel>(settings, ())
}
