//! Module defining the `Sniffer` struct, which trace gui's component statuses and permits
//! to share data among the different threads.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use iced::{window, Command};
use pcap::Device;
use rfd::FileHandle;

use crate::chart::manage_chart_data::update_charts_data;
use crate::gui::components::types::my_modal::MyModal;
use crate::gui::pages::types::running_page::RunningPage;
use crate::gui::pages::types::settings_page::SettingsPage;
use crate::gui::styles::types::custom_palette::{CustomPalette, ExtraStyles};
use crate::gui::styles::types::palette::Palette;
use crate::gui::types::message::Message;
use crate::gui::types::timing_events::TimingEvents;
use crate::mmdb::asn::ASN_MMDB;
use crate::mmdb::country::COUNTRY_MMDB;
use crate::mmdb::types::mmdb_reader::MmdbReader;
use crate::networking::manage_packets::get_capture_result;
use crate::networking::types::filters::Filters;
use crate::networking::types::host::Host;
use crate::networking::types::ip_collection::AddressCollection;
use crate::networking::types::my_device::MyDevice;
use crate::networking::types::my_link_type::MyLinkType;
use crate::networking::types::port_collection::PortCollection;
use crate::networking::types::search_parameters::SearchParameters;
use crate::notifications::notify_and_log::notify_and_log;
use crate::notifications::types::notifications::Notification;
use crate::notifications::types::sound::{play, Sound};
use crate::report::get_report_entries::get_searched_entries;
use crate::report::types::report_sort_type::ReportSortType;
use crate::secondary_threads::parse_packets::parse_packets;
use crate::translations::types::language::Language;
use crate::utils::types::file_info::FileInfo;
use crate::utils::types::web_page::WebPage;
use crate::{ConfigSettings, Configs, InfoTraffic, RunTimeData, StyleType, TrafficChart};

/// Struct on which the gui is based
///
/// It contains gui statuses and network traffic statistics to be shared among the different threads
pub struct Sniffer {
    /// Application's configurations: settings, window properties, name of last device sniffed
    pub configs: Arc<Mutex<Configs>>,
    /// Capture number, incremented at every new run
    pub current_capture_id: Arc<Mutex<usize>>,
    /// Capture data updated by thread parsing packets
    pub info_traffic: Arc<Mutex<InfoTraffic>>,
    /// Reports if a newer release of the software is available on GitHub
    pub newer_release_available: Arc<Mutex<Option<bool>>>,
    /// Traffic data displayed in GUI
    pub runtime_data: RunTimeData,
    /// Network adapter to be analyzed
    pub device: MyDevice,
    /// Active filters on the observed traffic
    pub filters: Filters,
    /// Signals if a pcap error occurred
    pub pcap_error: Option<String>,
    /// Waiting string
    pub waiting: String,
    /// Chart displayed
    pub traffic_chart: TrafficChart,
    /// Report type to be displayed
    pub report_sort_type: ReportSortType,
    /// Currently displayed modal; None if no modal is displayed
    pub modal: Option<MyModal>,
    /// Currently displayed settings page; None if settings is closed
    pub settings_page: Option<SettingsPage>,
    /// Remembers the last opened setting page
    pub last_opened_setting: SettingsPage,
    /// Defines the current running page
    pub running_page: RunningPage,
    /// Number of unread notifications
    pub unread_notifications: usize,
    /// Search parameters of inspect page
    pub search: SearchParameters,
    /// Current page number of inspect search results
    pub page_number: usize,
    /// MMDB reader for countries
    pub country_mmdb_reader: Arc<MmdbReader>,
    /// MMDB reader for ASN
    pub asn_mmdb_reader: Arc<MmdbReader>,
    /// Time-related events
    pub timing_events: TimingEvents,
}

impl Sniffer {
    pub fn new(
        configs: &Arc<Mutex<Configs>>,
        newer_release_available: Arc<Mutex<Option<bool>>>,
    ) -> Self {
        let ConfigSettings {
            style,
            language,
            mmdb_country,
            mmdb_asn,
            ..
        } = configs.lock().unwrap().settings.clone();
        let device = configs.lock().unwrap().device.to_my_device();
        Self {
            configs: configs.clone(),
            current_capture_id: Arc::new(Mutex::new(0)),
            info_traffic: Arc::new(Mutex::new(InfoTraffic::new())),
            newer_release_available,
            runtime_data: RunTimeData::new(),
            device,
            filters: Filters::default(),
            pcap_error: None,
            waiting: ".".to_string(),
            traffic_chart: TrafficChart::new(style, language),
            report_sort_type: ReportSortType::MostRecent,
            modal: None,
            settings_page: None,
            last_opened_setting: SettingsPage::Notifications,
            running_page: RunningPage::Init,
            unread_notifications: 0,
            search: SearchParameters::default(),
            page_number: 1,
            country_mmdb_reader: Arc::new(MmdbReader::from(&mmdb_country, COUNTRY_MMDB)),
            asn_mmdb_reader: Arc::new(MmdbReader::from(&mmdb_asn, ASN_MMDB)),
            timing_events: TimingEvents::default(),
        }
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::TickRun => return self.refresh_data(),
            Message::AdapterSelection(name) => self.set_adapter(&name),
            Message::IpVersionSelection(version, insert) => {
                if insert {
                    self.filters.ip_versions.insert(version);
                } else {
                    self.filters.ip_versions.remove(&version);
                }
            }
            Message::ProtocolSelection(protocol, insert) => {
                if insert {
                    self.filters.protocols.insert(protocol);
                } else {
                    self.filters.protocols.remove(&protocol);
                }
            }
            Message::AddressFilter(value) => {
                if let Some(collection) = AddressCollection::new(&value) {
                    self.filters.address_collection = collection;
                }
                self.filters.address_str = value;
            }
            Message::PortFilter(value) => {
                if let Some(collection) = PortCollection::new(&value) {
                    self.filters.port_collection = collection;
                }
                self.filters.port_str = value;
            }
            Message::ChartSelection(unit) => self.traffic_chart.change_kind(unit),
            Message::ReportSortSelection(sort) => self.report_sort_type = sort,
            Message::OpenWebPage(web_page) => Self::open_web(&web_page),
            Message::Start => self.start(),
            Message::Reset => return self.reset(),
            Message::Style(style) => {
                self.configs.lock().unwrap().settings.style = style;
                self.traffic_chart.change_style(style);
            }
            Message::LoadStyle(path) => {
                self.configs.lock().unwrap().settings.style_path = path.clone();
                if let Ok(palette) = Palette::from_file(path) {
                    let style = StyleType::Custom(ExtraStyles::CustomToml(
                        CustomPalette::from_palette(palette),
                    ));
                    self.configs.lock().unwrap().settings.style = style;
                    self.traffic_chart.change_style(style);
                }
            }
            Message::Waiting => self.update_waiting_dots(),
            Message::AddOrRemoveFavorite(host, add) => self.add_or_remove_favorite(&host, add),
            Message::ShowModal(modal) => {
                if self.settings_page.is_none() && self.modal.is_none() {
                    self.modal = Some(modal);
                }
            }
            Message::HideModal => self.modal = None,
            Message::OpenSettings(settings_page) => {
                if self.modal.is_none() {
                    self.settings_page = Some(settings_page);
                }
            }
            Message::OpenLastSettings => {
                if self.modal.is_none() && self.settings_page.is_none() {
                    self.settings_page = Some(self.last_opened_setting);
                }
            }
            Message::CloseSettings => self.close_settings(),
            Message::ChangeRunningPage(running_page) => {
                self.running_page = running_page;
                if running_page.eq(&RunningPage::Notifications) {
                    self.unread_notifications = 0;
                }
            }
            Message::LanguageSelection(language) => {
                self.configs.lock().unwrap().settings.language = language;
                self.traffic_chart.change_language(language);
            }
            Message::UpdateNotificationSettings(value, emit_sound) => {
                self.update_notification_settings(value, emit_sound);
            }
            Message::ChangeVolume(volume) => {
                play(Sound::Pop, volume);
                self.configs.lock().unwrap().settings.notifications.volume = volume;
            }
            Message::ClearAllNotifications => {
                self.runtime_data.logged_notifications = VecDeque::new();
                return self.update(Message::HideModal);
            }
            Message::SwitchPage(next) => {
                // To prevent SwitchPage be triggered when using `Alt` + `Tab` to switch back,
                // first check if user switch back just now, and ignore the request for a short time.
                if !self.timing_events.was_just_focus() {
                    self.switch_page(next);
                }
            }
            Message::ReturnKeyPressed => return self.shortcut_return(),
            Message::EscKeyPressed => return self.shortcut_esc(),
            Message::ResetButtonPressed => return self.reset_button_pressed(),
            Message::CtrlDPressed => return self.shortcut_ctrl_d(),
            Message::Search(parameters) => {
                self.page_number = 1;
                self.running_page = RunningPage::Inspect;
                self.search = parameters;
            }
            Message::UpdatePageNumber(increment) => {
                let new_page = if increment {
                    self.page_number.checked_add(1)
                } else {
                    self.page_number.checked_sub(1)
                }
                .unwrap();
                self.page_number = new_page;
            }
            Message::ArrowPressed(increment) => {
                if self.running_page.eq(&RunningPage::Inspect)
                    && self.settings_page.is_none()
                    && self.modal.is_none()
                {
                    if increment {
                        if self.page_number < (get_searched_entries(self).1 + 20 - 1) / 20 {
                            return self.update(Message::UpdatePageNumber(increment));
                        }
                    } else if self.page_number > 1 {
                        return self.update(Message::UpdatePageNumber(increment));
                    }
                }
            }
            Message::WindowFocused => self.timing_events.focus_now(),
            Message::GradientsSelection(gradient_type) => {
                self.configs.lock().unwrap().settings.color_gradient = gradient_type;
            }
            Message::ChangeScaleFactor(multiplier) => {
                self.configs.lock().unwrap().settings.scale_factor = multiplier;
            }
            Message::WindowMoved(x, y) => {
                self.configs.lock().unwrap().window.position = (x, y);
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Message::WindowResized(width, height) => {
                let scale_factor = self.configs.lock().unwrap().settings.scale_factor;
                let scaled_width = (f64::from(width) * scale_factor) as u32;
                let scaled_height = (f64::from(height) * scale_factor) as u32;
                self.configs.lock().unwrap().window.size = (scaled_width, scaled_height);
            }
            Message::CustomCountryDb(db) => {
                self.configs.lock().unwrap().settings.mmdb_country = db.clone();
                self.country_mmdb_reader = Arc::new(MmdbReader::from(&db, COUNTRY_MMDB));
            }
            Message::CustomAsnDb(db) => {
                self.configs.lock().unwrap().settings.mmdb_asn = db.clone();
                self.asn_mmdb_reader = Arc::new(MmdbReader::from(&db, ASN_MMDB));
            }
            // Message::CustomReport(path) => {
            //     self.settings.output_path = path;
            // }
            Message::CloseRequested => {
                self.configs.lock().unwrap().clone().store();
                return window::close();
            }
            Message::CopyIp(string) => {
                self.timing_events.copy_ip_now(string.clone());
                return iced::clipboard::write(string);
            }
            Message::OpenFile(old_file, file_info, consumer_message) => {
                return Command::perform(
                    Self::open_file(
                        old_file,
                        file_info,
                        self.configs.lock().unwrap().settings.language,
                    ),
                    consumer_message,
                );
            }
            Message::TickInit | Message::FontLoaded(_) => {}
        }
        Command::none()
    }

    fn refresh_data(&mut self) -> Command<Message> {
        let info_traffic_lock = self.info_traffic.lock().unwrap();
        self.runtime_data.all_packets = info_traffic_lock.all_packets;
        if info_traffic_lock.tot_received_packets + info_traffic_lock.tot_sent_packets == 0 {
            drop(info_traffic_lock);
            return self.update(Message::Waiting);
        }
        self.runtime_data.tot_sent_packets = info_traffic_lock.tot_sent_packets;
        self.runtime_data.tot_received_packets = info_traffic_lock.tot_received_packets;
        self.runtime_data.all_bytes = info_traffic_lock.all_bytes;
        self.runtime_data.tot_received_bytes = info_traffic_lock.tot_received_bytes;
        self.runtime_data.tot_sent_bytes = info_traffic_lock.tot_sent_bytes;
        self.runtime_data.dropped_packets = info_traffic_lock.dropped_packets;
        drop(info_traffic_lock);
        let emitted_notifications = notify_and_log(
            &mut self.runtime_data,
            self.configs.lock().unwrap().settings.notifications,
            &self.info_traffic.clone(),
        );
        self.info_traffic.lock().unwrap().favorites_last_interval = HashSet::new();
        self.runtime_data.tot_emitted_notifications += emitted_notifications;
        if self.running_page.ne(&RunningPage::Notifications) {
            self.unread_notifications += emitted_notifications;
        }
        update_charts_data(&mut self.runtime_data, &mut self.traffic_chart);

        let current_device_name = self.device.name.clone();
        // update ConfigDevice stored if different from last sniffed device
        let last_device_name_sniffed = self.configs.lock().unwrap().device.device_name.clone();
        if current_device_name.ne(&last_device_name_sniffed) {
            self.configs.lock().unwrap().device.device_name = current_device_name;
        }
        // waiting notifications
        if self.running_page.eq(&RunningPage::Notifications)
            && self.runtime_data.logged_notifications.is_empty()
        {
            return self.update(Message::Waiting);
        }
        Command::none()
    }

    fn open_web(web_page: &WebPage) {
        let url = web_page.get_url();
        #[cfg(target_os = "windows")]
        std::process::Command::new("explorer")
            .arg(url)
            .spawn()
            .unwrap();
        #[cfg(target_os = "macos")]
        std::process::Command::new("open").arg(url).spawn().unwrap();
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .unwrap();
    }

    fn start(&mut self) {
        let current_device_name = &*self.device.name.clone();
        self.set_adapter(current_device_name);
        let device = self.device.clone();
        let (pcap_error, cap_result) = get_capture_result(&device);
        self.pcap_error = pcap_error.clone();
        let info_traffic_mutex = self.info_traffic.clone();
        *info_traffic_mutex.lock().unwrap() = InfoTraffic::new();
        self.runtime_data = RunTimeData::new();
        let ConfigSettings {
            style, language, ..
        } = self.configs.lock().unwrap().settings;
        self.traffic_chart = TrafficChart::new(style, language);
        self.running_page = RunningPage::Overview;

        if pcap_error.is_none() {
            // no pcap error
            let cap = cap_result.unwrap();
            let current_capture_id = self.current_capture_id.clone();
            let filters = self.filters.clone();
            let country_mmdb_reader = self.country_mmdb_reader.clone();
            let asn_mmdb_reader = self.asn_mmdb_reader.clone();
            self.device.link_type = MyLinkType::from_pcap_link_type(cap.get_datalink());
            thread::Builder::new()
                .name("thread_parse_packets".to_string())
                .spawn(move || {
                    parse_packets(
                        &current_capture_id,
                        &device,
                        cap,
                        &filters,
                        &info_traffic_mutex,
                        &country_mmdb_reader,
                        &asn_mmdb_reader,
                    );
                })
                .unwrap();
        }
    }

    fn reset(&mut self) -> Command<Message> {
        self.running_page = RunningPage::Init;
        *self.current_capture_id.lock().unwrap() += 1; //change capture id to kill previous captures
        self.pcap_error = None;
        self.report_sort_type = ReportSortType::MostRecent;
        self.unread_notifications = 0;
        self.search = SearchParameters::default();
        self.page_number = 1;
        self.update(Message::HideModal)
    }

    fn set_adapter(&mut self, name: &str) {
        for dev in Device::list().expect("Error retrieving device list\r\n") {
            if dev.name.eq(&name) {
                let mut addresses_mutex = self.device.addresses.lock().unwrap();
                *addresses_mutex = dev.addresses;
                drop(addresses_mutex);
                self.device = MyDevice {
                    name: dev.name,
                    desc: dev.desc,
                    addresses: self.device.addresses.clone(),
                    link_type: MyLinkType::NotYetAssigned,
                };
                break;
            }
        }
    }

    fn update_waiting_dots(&mut self) {
        if self.waiting.len() > 2 {
            self.waiting = String::new();
        }
        self.waiting = ".".repeat(self.waiting.len() + 1);
    }

    fn add_or_remove_favorite(&mut self, host: &Host, add: bool) {
        let mut info_traffic = self.info_traffic.lock().unwrap();
        if add {
            info_traffic.favorite_hosts.insert(host.clone());
        } else {
            info_traffic.favorite_hosts.remove(host);
        }
        if let Some(host_info) = info_traffic.hosts.get_mut(host) {
            host_info.is_favorite = add;
        }
        drop(info_traffic);
    }

    fn close_settings(&mut self) {
        if let Some(page) = self.settings_page {
            self.last_opened_setting = page;
            self.settings_page = None;
        }
    }

    fn update_notification_settings(&mut self, value: Notification, emit_sound: bool) {
        let sound = match value {
            Notification::Packets(packets_notification) => {
                self.configs
                    .lock()
                    .unwrap()
                    .settings
                    .notifications
                    .packets_notification = packets_notification;
                packets_notification.sound
            }
            Notification::Bytes(bytes_notification) => {
                self.configs
                    .lock()
                    .unwrap()
                    .settings
                    .notifications
                    .bytes_notification = bytes_notification;
                bytes_notification.sound
            }
            Notification::Favorite(favorite_notification) => {
                self.configs
                    .lock()
                    .unwrap()
                    .settings
                    .notifications
                    .favorite_notification = favorite_notification;
                favorite_notification.sound
            }
        };
        if emit_sound {
            play(
                sound,
                self.configs.lock().unwrap().settings.notifications.volume,
            );
        }
    }

    fn switch_page(&mut self, next: bool) {
        match (self.running_page, self.settings_page, self.modal.is_none()) {
            (_, Some(current_setting), true) => {
                // Settings opened
                if next {
                    self.settings_page = Some(current_setting.next());
                } else {
                    self.settings_page = Some(current_setting.previous());
                }
            }
            (
                RunningPage::Inspect | RunningPage::Notifications | RunningPage::Overview,
                None,
                true,
            ) => {
                // Running with no overlays
                if self.runtime_data.tot_sent_packets + self.runtime_data.tot_received_packets > 0 {
                    // Running with no overlays and some packets filtered
                    self.running_page = if next {
                        self.running_page.next()
                    } else {
                        self.running_page.previous()
                    };
                    if self.running_page.eq(&RunningPage::Notifications) {
                        self.unread_notifications = 0;
                    }
                }
            }
            (_, _, _) => {}
        }
    }

    fn shortcut_return(&mut self) -> Command<Message> {
        if self.running_page.eq(&RunningPage::Init)
            && self.settings_page.is_none()
            && self.modal.is_none()
        {
            if self.filters.are_valid() {
                return self.update(Message::Start);
            }
        } else if self.modal.eq(&Some(MyModal::Quit)) {
            return self.update(Message::Reset);
        } else if self.modal.eq(&Some(MyModal::ClearAll)) {
            return self.update(Message::ClearAllNotifications);
        }
        Command::none()
    }

    fn shortcut_esc(&mut self) -> Command<Message> {
        if self.modal.is_some() {
            return self.update(Message::HideModal);
        } else if self.settings_page.is_some() {
            return self.update(Message::CloseSettings);
        }
        Command::none()
    }

    // also called when backspace key is pressed on a running state
    fn reset_button_pressed(&mut self) -> Command<Message> {
        if self.running_page.ne(&RunningPage::Init) {
            return if self.info_traffic.lock().unwrap().all_packets == 0
                && self.settings_page.is_none()
            {
                self.update(Message::Reset)
            } else {
                self.update(Message::ShowModal(MyModal::Quit))
            };
        }
        Command::none()
    }

    fn shortcut_ctrl_d(&mut self) -> Command<Message> {
        if self.running_page.eq(&RunningPage::Notifications)
            && !self.runtime_data.logged_notifications.is_empty()
        {
            return self.update(Message::ShowModal(MyModal::ClearAll));
        }
        Command::none()
    }

    async fn open_file(old_file: String, file_info: FileInfo, language: Language) -> String {
        let starting_directory = if old_file.is_empty() {
            std::env::var("HOME").unwrap_or_default()
        } else {
            let mut folder_path = PathBuf::from(&old_file);
            folder_path.pop();
            folder_path.to_string_lossy().to_string()
        };
        let picked_file = rfd::AsyncFileDialog::new()
            .set_title(file_info.action_info(language))
            .add_filter(file_info.get_extension(), &[file_info.get_extension()])
            .set_directory(starting_directory)
            .pick_file()
            .await
            .unwrap_or_else(|| FileHandle::from(PathBuf::from(&old_file)));

        picked_file.path().to_string_lossy().to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_must_use)]

    use std::collections::{HashSet, VecDeque};
    use std::fs::remove_file;
    use std::ops::Sub;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use serial_test::{parallel, serial};

    use crate::countries::types::country::Country;
    use crate::gui::components::types::my_modal::MyModal;
    use crate::gui::pages::types::settings_page::SettingsPage;
    use crate::gui::styles::types::custom_palette::ExtraStyles;
    use crate::gui::styles::types::gradient_type::GradientType;
    use crate::gui::types::message::Message;
    use crate::networking::types::host::Host;
    use crate::notifications::types::logged_notification::{
        LoggedNotification, PacketsThresholdExceeded,
    };
    use crate::notifications::types::notifications::{
        BytesNotification, FavoriteNotification, Notification, Notifications, PacketsNotification,
    };
    use crate::notifications::types::sound::Sound;
    use crate::{
        ByteMultiple, ChartType, ConfigDevice, ConfigSettings, ConfigWindow, Configs, IpVersion,
        Language, Protocol, ReportSortType, RunningPage, Sniffer, StyleType,
    };

    // tests using this will require the #[parallel] annotation
    fn new_sniffer() -> Sniffer {
        Sniffer::new(
            &Arc::new(Mutex::new(Configs::default())),
            Arc::new(Mutex::new(None)),
        )
    }

    // tests using this will require the #[serial] annotation
    fn new_sniffer_with_configs(configs: Configs) -> Sniffer {
        Sniffer::new(&Arc::new(Mutex::new(configs)), Arc::new(Mutex::new(None)))
    }

    // helpful to clean up files generated from tests
    impl Drop for Sniffer {
        fn drop(&mut self) {
            let settings_path_str = ConfigSettings::test_path();
            let settings_path = Path::new(&settings_path_str);
            if settings_path.exists() {
                remove_file(ConfigSettings::test_path()).unwrap();
            }

            let device_path_str = ConfigDevice::test_path();
            let device_path = Path::new(&device_path_str);
            if device_path.exists() {
                remove_file(ConfigDevice::test_path()).unwrap();
            }

            let window_path_str = ConfigWindow::test_path();
            let window_path = Path::new(&window_path_str);
            if window_path.exists() {
                remove_file(ConfigWindow::test_path()).unwrap();
            }
        }
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_update_ip_version() {
        let mut sniffer = new_sniffer();

        assert_eq!(sniffer.filters.ip_versions, HashSet::from(IpVersion::ALL));
        sniffer.update(Message::IpVersionSelection(IpVersion::IPv6, true));
        assert_eq!(sniffer.filters.ip_versions, HashSet::from(IpVersion::ALL));
        sniffer.update(Message::IpVersionSelection(IpVersion::IPv4, false));
        assert_eq!(
            sniffer.filters.ip_versions,
            HashSet::from([IpVersion::IPv6])
        );
        sniffer.update(Message::IpVersionSelection(IpVersion::IPv6, false));
        assert_eq!(sniffer.filters.ip_versions, HashSet::new());
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_update_protocol() {
        let mut sniffer = new_sniffer();

        assert_eq!(sniffer.filters.protocols, HashSet::from(Protocol::ALL));
        sniffer.update(Message::ProtocolSelection(Protocol::UDP, true));
        assert_eq!(sniffer.filters.protocols, HashSet::from(Protocol::ALL));
        sniffer.update(Message::ProtocolSelection(Protocol::UDP, false));
        assert_eq!(
            sniffer.filters.protocols,
            HashSet::from([Protocol::TCP, Protocol::ICMP])
        );
        sniffer.update(Message::ProtocolSelection(Protocol::TCP, false));
        assert_eq!(sniffer.filters.protocols, HashSet::from([Protocol::ICMP]));
        sniffer.update(Message::ProtocolSelection(Protocol::ICMP, false));
        assert_eq!(sniffer.filters.protocols, HashSet::new());
        sniffer.update(Message::ProtocolSelection(Protocol::UDP, true));
        assert_eq!(sniffer.filters.protocols, HashSet::from([Protocol::UDP]));
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_update_chart_kind() {
        let mut sniffer = new_sniffer();

        assert_eq!(sniffer.traffic_chart.chart_type, ChartType::Bytes);
        sniffer.update(Message::ChartSelection(ChartType::Packets));
        assert_eq!(sniffer.traffic_chart.chart_type, ChartType::Packets);
        sniffer.update(Message::ChartSelection(ChartType::Packets));
        assert_eq!(sniffer.traffic_chart.chart_type, ChartType::Packets);
        sniffer.update(Message::ChartSelection(ChartType::Bytes));
        assert_eq!(sniffer.traffic_chart.chart_type, ChartType::Bytes);
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_update_report_kind() {
        let mut sniffer = new_sniffer();

        assert_eq!(sniffer.report_sort_type, ReportSortType::MostRecent);
        sniffer.update(Message::ReportSortSelection(ReportSortType::MostBytes));
        assert_eq!(sniffer.report_sort_type, ReportSortType::MostBytes);
        sniffer.update(Message::ReportSortSelection(ReportSortType::MostPackets));
        assert_eq!(sniffer.report_sort_type, ReportSortType::MostPackets);
        sniffer.update(Message::ReportSortSelection(ReportSortType::MostPackets));
        assert_eq!(sniffer.report_sort_type, ReportSortType::MostPackets);
        sniffer.update(Message::ReportSortSelection(ReportSortType::MostRecent));
        assert_eq!(sniffer.report_sort_type, ReportSortType::MostRecent);
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_update_style() {
        let mut sniffer = new_sniffer();

        sniffer.update(Message::Style(StyleType::MonAmour));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.style,
            StyleType::MonAmour
        );
        sniffer.update(Message::Style(StyleType::Day));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.style,
            StyleType::Day
        );
        sniffer.update(Message::Style(StyleType::Night));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.style,
            StyleType::Night
        );
        sniffer.update(Message::Style(StyleType::DeepSea));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.style,
            StyleType::DeepSea
        );
        sniffer.update(Message::Style(StyleType::DeepSea));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.style,
            StyleType::DeepSea
        );
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_waiting_dots_update() {
        let mut sniffer = new_sniffer();

        assert_eq!(sniffer.waiting, ".".to_string());
        sniffer.update(Message::Waiting);
        assert_eq!(sniffer.waiting, "..".to_string());

        sniffer.update(Message::Waiting);
        assert_eq!(sniffer.waiting, "...".to_string());

        sniffer.update(Message::Waiting);
        assert_eq!(sniffer.waiting, ".".to_string());
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_modify_favorite_connections() {
        let mut sniffer = new_sniffer();
        // remove 1
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "1.1".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            false,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::new()
        );
        // remove 2
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "2.2".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            false,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::new()
        );
        // add 2
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "2.2".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            true,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::from([Host {
                domain: "2.2".to_string(),
                asn: Default::default(),
                country: Country::US,
            }])
        );
        // remove 1
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "1.1".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            false,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::from([Host {
                domain: "2.2".to_string(),
                asn: Default::default(),
                country: Country::US,
            }])
        );
        // add 2
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "2.2".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            true,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::from([Host {
                domain: "2.2".to_string(),
                asn: Default::default(),
                country: Country::US,
            }])
        );
        // add 1
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "1.1".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            true,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::from([
                Host {
                    domain: "1.1".to_string(),
                    asn: Default::default(),
                    country: Country::US,
                },
                Host {
                    domain: "2.2".to_string(),
                    asn: Default::default(),
                    country: Country::US,
                }
            ])
        );
        // add 3
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "3.3".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            true,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::from([
                Host {
                    domain: "1.1".to_string(),
                    asn: Default::default(),
                    country: Country::US,
                },
                Host {
                    domain: "2.2".to_string(),
                    asn: Default::default(),
                    country: Country::US,
                },
                Host {
                    domain: "3.3".to_string(),
                    asn: Default::default(),
                    country: Country::US,
                }
            ])
        );
        // remove 2
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "2.2".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            false,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::from([
                Host {
                    domain: "1.1".to_string(),
                    asn: Default::default(),
                    country: Country::US,
                },
                Host {
                    domain: "3.3".to_string(),
                    asn: Default::default(),
                    country: Country::US,
                }
            ])
        );
        // remove 3
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "3.3".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            false,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::from([Host {
                domain: "1.1".to_string(),
                asn: Default::default(),
                country: Country::US,
            }])
        );
        // remove 1
        sniffer.update(Message::AddOrRemoveFavorite(
            Host {
                domain: "1.1".to_string(),
                asn: Default::default(),
                country: Country::US,
            },
            false,
        ));
        assert_eq!(
            sniffer.info_traffic.lock().unwrap().favorite_hosts,
            HashSet::new()
        );
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_show_and_hide_modal_and_settings() {
        let mut sniffer = new_sniffer();

        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Notifications);
        // open settings
        sniffer.update(Message::OpenLastSettings);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Notifications));
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Notifications);
        // switch settings page
        sniffer.update(Message::OpenSettings(SettingsPage::Appearance));
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Appearance));
        sniffer.update(Message::OpenSettings(SettingsPage::General));
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::General));
        // try opening modal with settings opened
        sniffer.update(Message::ShowModal(MyModal::Quit));
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::General));
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Notifications);
        // close settings
        sniffer.update(Message::CloseSettings);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::General);
        // reopen settings
        sniffer.update(Message::OpenLastSettings);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::General));
        assert_eq!(sniffer.last_opened_setting, SettingsPage::General);
        // switch settings page
        sniffer.update(Message::OpenSettings(SettingsPage::Appearance));
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Appearance));
        // close settings
        sniffer.update(Message::CloseSettings);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);

        // open clear all modal
        sniffer.update(Message::ShowModal(MyModal::ClearAll));
        assert_eq!(sniffer.modal, Some(MyModal::ClearAll));
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);
        // try opening settings with clear all modal opened
        sniffer.update(Message::OpenLastSettings);
        assert_eq!(sniffer.modal, Some(MyModal::ClearAll));
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);
        // try opening quit modal with clear all modal opened
        sniffer.update(Message::ShowModal(MyModal::Quit));
        assert_eq!(sniffer.modal, Some(MyModal::ClearAll));
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);
        // close clear all modal
        sniffer.update(Message::HideModal);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);

        // open quit modal
        sniffer.update(Message::ShowModal(MyModal::Quit));
        assert_eq!(sniffer.modal, Some(MyModal::Quit));
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);
        // try opening settings with clear all modal opened
        sniffer.update(Message::OpenLastSettings);
        assert_eq!(sniffer.modal, Some(MyModal::Quit));
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);
        // try opening clear all modal with quit modal opened
        sniffer.update(Message::ShowModal(MyModal::ClearAll));
        assert_eq!(sniffer.modal, Some(MyModal::Quit));
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);
        // close quit modal
        sniffer.update(Message::HideModal);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.last_opened_setting, SettingsPage::Appearance);
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_update_language() {
        let mut sniffer = new_sniffer();

        assert_eq!(
            sniffer.configs.lock().unwrap().settings.language,
            Language::EN
        );
        assert_eq!(sniffer.traffic_chart.language, Language::EN);
        sniffer.update(Message::LanguageSelection(Language::IT));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.language,
            Language::IT
        );
        assert_eq!(sniffer.traffic_chart.language, Language::IT);
        sniffer.update(Message::LanguageSelection(Language::IT));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.language,
            Language::IT
        );
        assert_eq!(sniffer.traffic_chart.language, Language::IT);
        sniffer.update(Message::LanguageSelection(Language::ZH));
        assert_eq!(
            sniffer.configs.lock().unwrap().settings.language,
            Language::ZH
        );
        assert_eq!(sniffer.traffic_chart.language, Language::ZH);
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_update_notification_settings() {
        let mut sniffer = new_sniffer();

        // initial default state
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .volume,
            60
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .packets_notification,
            PacketsNotification {
                threshold: None,
                sound: Sound::Gulp,
                previous_threshold: 750
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .bytes_notification,
            BytesNotification {
                threshold: None,
                byte_multiple: ByteMultiple::KB,
                sound: Sound::Pop,
                previous_threshold: 800000
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .favorite_notification,
            FavoriteNotification {
                notify_on_favorite: false,
                sound: Sound::Swhoosh,
            }
        );
        // change volume
        sniffer.update(Message::ChangeVolume(95));
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .volume,
            95
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .packets_notification,
            PacketsNotification {
                threshold: None,
                sound: Sound::Gulp,
                previous_threshold: 750
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .bytes_notification,
            BytesNotification {
                threshold: None,
                byte_multiple: ByteMultiple::KB,
                sound: Sound::Pop,
                previous_threshold: 800000
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .favorite_notification,
            FavoriteNotification {
                notify_on_favorite: false,
                sound: Sound::Swhoosh,
            }
        );
        // change packets notifications
        sniffer.update(Message::UpdateNotificationSettings(
            Notification::Packets(PacketsNotification {
                threshold: Some(1122),
                sound: Sound::None,
                previous_threshold: 1122,
            }),
            false,
        ));
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .volume,
            95
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .packets_notification,
            PacketsNotification {
                threshold: Some(1122),
                sound: Sound::None,
                previous_threshold: 1122
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .bytes_notification,
            BytesNotification {
                threshold: None,
                byte_multiple: ByteMultiple::KB,
                sound: Sound::Pop,
                previous_threshold: 800000
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .favorite_notification,
            FavoriteNotification {
                notify_on_favorite: false,
                sound: Sound::Swhoosh,
            }
        );
        // change bytes notifications
        sniffer.update(Message::UpdateNotificationSettings(
            Notification::Bytes(BytesNotification {
                threshold: Some(3),
                byte_multiple: ByteMultiple::GB,
                sound: Sound::None,
                previous_threshold: 3,
            }),
            true,
        ));
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .volume,
            95
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .packets_notification,
            PacketsNotification {
                threshold: Some(1122),
                sound: Sound::None,
                previous_threshold: 1122
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .bytes_notification,
            BytesNotification {
                threshold: Some(3),
                byte_multiple: ByteMultiple::GB,
                sound: Sound::None,
                previous_threshold: 3,
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .favorite_notification,
            FavoriteNotification {
                notify_on_favorite: false,
                sound: Sound::Swhoosh,
            }
        );
        // change favorite notifications
        sniffer.update(Message::UpdateNotificationSettings(
            Notification::Favorite(FavoriteNotification {
                notify_on_favorite: true,
                sound: Sound::Pop,
            }),
            true,
        ));
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .volume,
            95
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .packets_notification,
            PacketsNotification {
                threshold: Some(1122),
                sound: Sound::None,
                previous_threshold: 1122
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .bytes_notification,
            BytesNotification {
                threshold: Some(3),
                byte_multiple: ByteMultiple::GB,
                sound: Sound::None,
                previous_threshold: 3,
            }
        );
        assert_eq!(
            sniffer
                .configs
                .lock()
                .unwrap()
                .settings
                .notifications
                .favorite_notification,
            FavoriteNotification {
                notify_on_favorite: true,
                sound: Sound::Pop
            }
        );
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_clear_all_notifications() {
        let mut sniffer = new_sniffer();
        sniffer.runtime_data.logged_notifications =
            VecDeque::from([LoggedNotification::PacketsThresholdExceeded(
                PacketsThresholdExceeded {
                    threshold: 0,
                    incoming: 0,
                    outgoing: 0,
                    timestamp: "".to_string(),
                },
            )]);

        assert_eq!(sniffer.modal, None);
        sniffer.update(Message::ShowModal(MyModal::ClearAll));
        assert_eq!(sniffer.modal, Some(MyModal::ClearAll));
        assert_eq!(sniffer.runtime_data.logged_notifications.len(), 1);
        sniffer.update(Message::ClearAllNotifications);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.runtime_data.logged_notifications.len(), 0);
    }

    #[test]
    #[parallel] // needed to not collide with other tests generating configs files
    fn test_correctly_switch_running_and_settings_pages() {
        let mut sniffer = new_sniffer();
        sniffer.timing_events.focus = std::time::Instant::now().sub(Duration::from_millis(400));

        // initial status
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.running_page, RunningPage::Init);
        // nothing changes
        sniffer.update(Message::SwitchPage(true));
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.running_page, RunningPage::Init);
        // switch settings
        sniffer.update(Message::OpenLastSettings);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Notifications));
        assert_eq!(sniffer.running_page, RunningPage::Init);
        sniffer.update(Message::SwitchPage(false));
        assert_eq!(sniffer.settings_page, Some(SettingsPage::General));
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.running_page, RunningPage::Init);
        sniffer.update(Message::SwitchPage(true));
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Notifications));
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.running_page, RunningPage::Init);
        sniffer.update(Message::CloseSettings);
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.running_page, RunningPage::Init);
        // change state to running
        sniffer.running_page = RunningPage::Overview;
        assert_eq!(sniffer.settings_page, None);
        assert_eq!(sniffer.modal, None);
        assert_eq!(sniffer.running_page, RunningPage::Overview);
        // switch with closed setting and no packets received => nothing changes
        sniffer.update(Message::SwitchPage(true));
        assert_eq!(sniffer.running_page, RunningPage::Overview);
        assert_eq!(sniffer.settings_page, None);
        // switch with closed setting and some packets received => change running page
        sniffer.runtime_data.tot_received_packets += 1;
        sniffer.update(Message::SwitchPage(true));
        assert_eq!(sniffer.running_page, RunningPage::Inspect);
        assert_eq!(sniffer.settings_page, None);
        // switch with opened settings => change settings
        sniffer.update(Message::OpenLastSettings);
        assert_eq!(sniffer.running_page, RunningPage::Inspect);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Notifications));
        sniffer.update(Message::SwitchPage(true));
        assert_eq!(sniffer.running_page, RunningPage::Inspect);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Appearance));

        // focus the window and try to switch => nothing changes
        sniffer.update(Message::WindowFocused);
        sniffer.update(Message::SwitchPage(true));
        assert_eq!(sniffer.running_page, RunningPage::Inspect);
        assert_eq!(sniffer.settings_page, Some(SettingsPage::Appearance));
    }

    #[test]
    #[serial] // needed to not collide with other tests generating configs files
    fn test_config_settings() {
        let path_string = ConfigSettings::test_path();
        let path = Path::new(&path_string);

        assert!(!path.exists());

        let mut sniffer = new_sniffer_with_configs(Configs::load());

        assert!(path.exists());

        // check that the current settings are the default ones
        let settings_start = sniffer.configs.lock().unwrap().settings.clone();
        assert_eq!(
            settings_start,
            ConfigSettings {
                color_gradient: GradientType::None,
                language: Language::EN,
                scale_factor: 1.0,
                mmdb_country: "".to_string(),
                mmdb_asn: "".to_string(),
                style_path: "".to_string(),
                notifications: Notifications {
                    volume: 60,
                    packets_notification: Default::default(),
                    bytes_notification: Default::default(),
                    favorite_notification: Default::default()
                },
                style: StyleType::Night
            }
        );

        // change some configs by sending messages
        sniffer.update(Message::GradientsSelection(GradientType::Wild));
        sniffer.update(Message::LanguageSelection(Language::ZH));
        sniffer.update(Message::ChangeScaleFactor(0.65));
        sniffer.update(Message::CustomCountryDb("countrymmdb".to_string()));
        sniffer.update(Message::CustomAsnDb("asnmmdb".to_string()));
        sniffer.update(Message::LoadStyle(format!(
            "{}/resources/themes/catppuccin.toml",
            env!("CARGO_MANIFEST_DIR")
        )));
        sniffer.update(Message::Style(StyleType::Custom(ExtraStyles::DraculaDark)));
        sniffer.update(Message::ChangeVolume(100));

        // quit the app by sending a CloseRequested message
        sniffer.update(Message::CloseRequested);

        assert!(path.exists());

        // check that updated configs are inherited by a new sniffer instance
        let settings_end = new_sniffer_with_configs(Configs::load())
            .configs
            .lock()
            .unwrap()
            .settings
            .clone();
        assert_eq!(
            settings_end,
            ConfigSettings {
                color_gradient: GradientType::Wild,
                language: Language::ZH,
                scale_factor: 0.65,
                mmdb_country: "countrymmdb".to_string(),
                mmdb_asn: "asnmmdb".to_string(),
                style_path: format!(
                    "{}/resources/themes/catppuccin.toml",
                    env!("CARGO_MANIFEST_DIR")
                ),
                notifications: Notifications {
                    volume: 100,
                    packets_notification: Default::default(),
                    bytes_notification: Default::default(),
                    favorite_notification: Default::default()
                },
                style: StyleType::Custom(ExtraStyles::DraculaDark)
            }
        );
    }

    #[test]
    #[serial] // needed to not collide with other tests generating configs files
    fn test_config_window() {
        let path_string = ConfigWindow::test_path();
        let path = Path::new(&path_string);

        assert!(!path.exists());

        let mut sniffer = new_sniffer_with_configs(Configs::load());

        assert!(path.exists());

        // check that the current window properties are the default ones
        let window_start = sniffer.configs.lock().unwrap().window;
        assert_eq!(
            window_start,
            ConfigWindow {
                position: (0, 0),
                size: (1190, 670),
            }
        );

        // change window properties by sending messages
        sniffer.update(Message::WindowMoved(-10, 555));
        sniffer.update(Message::WindowResized(1000, 999));

        // quit the app by sending a CloseRequested message
        sniffer.update(Message::CloseRequested);

        assert!(path.exists());

        // check that updated configs are inherited by a new sniffer instance
        let window_end = new_sniffer_with_configs(Configs::load())
            .configs
            .lock()
            .unwrap()
            .window
            .clone();
        assert_eq!(
            window_end,
            ConfigWindow {
                position: (-10, 555),
                size: (1000, 999),
            }
        );
    }
}
