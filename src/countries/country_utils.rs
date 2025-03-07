use iced::widget::svg::Handle;
use iced::widget::tooltip::Position;
use iced::widget::Svg;
use iced::widget::Tooltip;
use iced::{Font, Length, Renderer};

use crate::countries::flags_pictures::{
    AD, AE, AF, AG, AI, AL, AM, AO, AQ, AR, AS, AT, AU, AW, AX, AZ, BA, BB, BD, BE, BF, BG, BH, BI,
    BJ, BM, BN, BO, BR, BROADCAST, BS, BT, BV, BW, BY, BZ, CA, CC, CD, CF, CG, CH, CI, CK, CL, CM,
    CN, CO, COMPUTER, CR, CU, CV, CW, CX, CY, CZ, DE, DJ, DK, DM, DO, DZ, EC, EE, EG, EH, ER, ES,
    ET, FI, FJ, FK, FLAGS_WIDTH_BIG, FLAGS_WIDTH_SMALL, FM, FO, FR, GA, GB, GD, GE, GG, GH, GI, GL,
    GM, GN, GQ, GR, GS, GT, GU, GW, GY, HK, HN, HOME, HR, HT, HU, ID, IE, IL, IM, IN, IO, IQ, IR,
    IS, IT, JE, JM, JO, JP, KE, KG, KH, KI, KM, KN, KP, KR, KW, KY, KZ, LA, LB, LC, LI, LK, LR, LS,
    LT, LU, LV, LY, MA, MC, MD, ME, MG, MH, MK, ML, MM, MN, MO, MP, MR, MS, MT, MU, MULTICAST, MV,
    MW, MX, MY, MZ, NA, NC, NE, NF, NG, NI, NL, NO, NP, NR, NU, NZ, OM, PA, PE, PF, PG, PH, PK, PL,
    PN, PR, PS, PT, PW, PY, QA, RO, RS, RU, RW, SA, SB, SC, SD, SE, SG, SH, SI, SK, SL, SM, SN, SO,
    SR, SS, ST, SV, SX, SY, SZ, TC, TD, TF, TG, TH, TJ, TK, TL, TM, TN, TO, TR, TT, TV, TW, TZ, UA,
    UG, UNKNOWN, US, UY, UZ, VA, VC, VE, VG, VI, VN, VU, WS, YE, ZA, ZM, ZW,
};
use crate::countries::types::country::Country;
use crate::gui::styles::container::ContainerType;
use crate::gui::styles::svg::SvgType;
use crate::gui::types::message::Message;
use crate::networking::types::data_info_host::DataInfoHost;
use crate::networking::types::traffic_type::TrafficType;
use crate::translations::translations_2::{
    local_translation, unknown_translation, your_network_adapter_translation,
};
use crate::{Language, StyleType};

fn get_flag_from_country(
    country: Country,
    width: f32,
    is_local: bool,
    is_loopback: bool,
    traffic_type: TrafficType,
    language: Language,
) -> (Svg<Renderer<StyleType>>, String) {
    #![allow(clippy::too_many_lines)]
    let mut tooltip = country.to_string();
    let mut svg_style = SvgType::Standard;
    let svg = Svg::new(Handle::from_memory(Vec::from(match country {
        Country::AD => AD,
        Country::AE => AE,
        Country::AF => AF,
        Country::AG => AG,
        Country::AI => AI,
        Country::AL => AL,
        Country::AM => AM,
        Country::AO => AO,
        Country::AQ => AQ,
        Country::AR => AR,
        Country::AS => AS,
        Country::AT => AT,
        Country::AU | Country::HM => AU,
        Country::AW => AW,
        Country::AX => AX,
        Country::AZ => AZ,
        Country::BA => BA,
        Country::BB => BB,
        Country::BD => BD,
        Country::BE => BE,
        Country::BF => BF,
        Country::BG => BG,
        Country::BH => BH,
        Country::BI => BI,
        Country::BJ => BJ,
        Country::BM => BM,
        Country::BN => BN,
        Country::BO => BO,
        Country::BR => BR,
        Country::BS => BS,
        Country::BT => BT,
        Country::BV => BV,
        Country::BW => BW,
        Country::BY => BY,
        Country::BZ => BZ,
        Country::CA => CA,
        Country::CC => CC,
        Country::CD => CD,
        Country::CF => CF,
        Country::CG => CG,
        Country::CH => CH,
        Country::CI => CI,
        Country::CK => CK,
        Country::CL => CL,
        Country::CM => CM,
        Country::CN => CN,
        Country::CO => CO,
        Country::CR => CR,
        Country::CU => CU,
        Country::CV => CV,
        Country::CW => CW,
        Country::CX => CX,
        Country::CY => CY,
        Country::CZ => CZ,
        Country::DE => DE,
        Country::DJ => DJ,
        Country::DK => DK,
        Country::DM => DM,
        Country::DO => DO,
        Country::DZ => DZ,
        Country::EC => EC,
        Country::EE => EE,
        Country::EG => EG,
        Country::EH => EH,
        Country::ER => ER,
        Country::ES => ES,
        Country::ET => ET,
        Country::FI => FI,
        Country::FJ => FJ,
        Country::FK => FK,
        Country::FM => FM,
        Country::FO => FO,
        Country::FR
        | Country::BL
        | Country::GF
        | Country::GP
        | Country::MF
        | Country::MQ
        | Country::PM
        | Country::RE
        | Country::WF
        | Country::YT => FR,
        Country::GA => GA,
        Country::GB => GB,
        Country::GD => GD,
        Country::GE => GE,
        Country::GG => GG,
        Country::GH => GH,
        Country::GI => GI,
        Country::GL => GL,
        Country::GM => GM,
        Country::GN => GN,
        Country::GQ => GQ,
        Country::GR => GR,
        Country::GS => GS,
        Country::GT => GT,
        Country::GU => GU,
        Country::GW => GW,
        Country::GY => GY,
        Country::HK => HK,
        Country::HN => HN,
        Country::HR => HR,
        Country::HT => HT,
        Country::HU => HU,
        Country::ID => ID,
        Country::IE => IE,
        Country::IL => IL,
        Country::IM => IM,
        Country::IN => IN,
        Country::IO => IO,
        Country::IQ => IQ,
        Country::IR => IR,
        Country::IS => IS,
        Country::IT => IT,
        Country::JE => JE,
        Country::JM => JM,
        Country::JO => JO,
        Country::JP => JP,
        Country::KE => KE,
        Country::KG => KG,
        Country::KH => KH,
        Country::KI => KI,
        Country::KM => KM,
        Country::KN => KN,
        Country::KP => KP,
        Country::KR => KR,
        Country::KW => KW,
        Country::KY => KY,
        Country::KZ => KZ,
        Country::LA => LA,
        Country::LB => LB,
        Country::LC => LC,
        Country::LI => LI,
        Country::LK => LK,
        Country::LR => LR,
        Country::LS => LS,
        Country::LT => LT,
        Country::LU => LU,
        Country::LV => LV,
        Country::LY => LY,
        Country::MA => MA,
        Country::MC => MC,
        Country::MD => MD,
        Country::ME => ME,
        Country::MG => MG,
        Country::MH => MH,
        Country::MK => MK,
        Country::ML => ML,
        Country::MM => MM,
        Country::MN => MN,
        Country::MO => MO,
        Country::MP => MP,
        Country::MR => MR,
        Country::MS => MS,
        Country::MT => MT,
        Country::MU => MU,
        Country::MV => MV,
        Country::MW => MW,
        Country::MX => MX,
        Country::MY => MY,
        Country::MZ => MZ,
        Country::NA => NA,
        Country::NC => NC,
        Country::NE => NE,
        Country::NF => NF,
        Country::NG => NG,
        Country::NI => NI,
        Country::NL | Country::BQ => NL,
        Country::NO | Country::SJ => NO,
        Country::NP => NP,
        Country::NR => NR,
        Country::NU => NU,
        Country::NZ => NZ,
        Country::OM => OM,
        Country::PA => PA,
        Country::PE => PE,
        Country::PF => PF,
        Country::PG => PG,
        Country::PH => PH,
        Country::PK => PK,
        Country::PL => PL,
        Country::PN => PN,
        Country::PR => PR,
        Country::PS => PS,
        Country::PT => PT,
        Country::PW => PW,
        Country::PY => PY,
        Country::QA => QA,
        Country::RO => RO,
        Country::RS => RS,
        Country::RU => RU,
        Country::RW => RW,
        Country::SA => SA,
        Country::SB => SB,
        Country::SC => SC,
        Country::SD => SD,
        Country::SE => SE,
        Country::SG => SG,
        Country::SH => SH,
        Country::SI => SI,
        Country::SK => SK,
        Country::SL => SL,
        Country::SM => SM,
        Country::SN => SN,
        Country::SO => SO,
        Country::SR => SR,
        Country::SS => SS,
        Country::ST => ST,
        Country::SV => SV,
        Country::SX => SX,
        Country::SY => SY,
        Country::SZ => SZ,
        Country::TC => TC,
        Country::TD => TD,
        Country::TF => TF,
        Country::TG => TG,
        Country::TH => TH,
        Country::TJ => TJ,
        Country::TK => TK,
        Country::TL => TL,
        Country::TM => TM,
        Country::TN => TN,
        Country::TO => TO,
        Country::TR => TR,
        Country::TT => TT,
        Country::TV => TV,
        Country::TW => TW,
        Country::TZ => TZ,
        Country::UA => UA,
        Country::UG => UG,
        Country::US | Country::UM => US,
        Country::UY => UY,
        Country::UZ => UZ,
        Country::VA => VA,
        Country::VC => VC,
        Country::VE => VE,
        Country::VG => VG,
        Country::VI => VI,
        Country::VN => VN,
        Country::VU => VU,
        Country::WS => WS,
        Country::YE => YE,
        Country::ZA => ZA,
        Country::ZM => ZM,
        Country::ZW => ZW,
        Country::ZZ => {
            if is_loopback {
                tooltip = your_network_adapter_translation(language);
                svg_style = SvgType::AdaptColor;
                COMPUTER
            } else if is_local {
                tooltip = local_translation(language);
                svg_style = SvgType::AdaptColor;
                HOME
            } else if traffic_type.eq(&TrafficType::Multicast) {
                tooltip = "Multicast".to_string();
                svg_style = SvgType::AdaptColor;
                MULTICAST
            } else if traffic_type.eq(&TrafficType::Broadcast) {
                tooltip = "Broadcast".to_string();
                svg_style = SvgType::AdaptColor;
                BROADCAST
            } else {
                tooltip = unknown_translation(language);
                svg_style = SvgType::AdaptColor;
                UNKNOWN
            }
        }
    })))
    .style(svg_style)
    .width(Length::Fixed(width))
    .height(Length::Fixed(width * 0.75));

    (svg, tooltip)
}

pub fn get_flag_tooltip(
    country: Country,
    width: f32,
    host_info: &DataInfoHost,
    language: Language,
    font: Font,
) -> Tooltip<'static, Message, Renderer<StyleType>> {
    let is_local = host_info.is_local;
    let is_loopback = host_info.is_loopback;
    let traffic_type = host_info.traffic_type;
    let (content, tooltip) = get_flag_from_country(
        country,
        width,
        is_local,
        is_loopback,
        traffic_type,
        language,
    );

    let mut tooltip = Tooltip::new(content, tooltip, Position::FollowCursor)
        .font(font)
        .snap_within_viewport(true)
        .style(ContainerType::Tooltip);

    if width == FLAGS_WIDTH_SMALL {
        tooltip = tooltip.padding(3);
    }

    tooltip
}

pub fn get_computer_tooltip(
    is_my_address: bool,
    is_local: bool,
    traffic_type: TrafficType,
    language: Language,
    font: Font,
) -> Tooltip<'static, Message, Renderer<StyleType>> {
    let content = Svg::new(Handle::from_memory(Vec::from(
        match (is_my_address, is_local, traffic_type) {
            (true, _, _) => COMPUTER,
            (false, true, _) => HOME,
            (false, false, TrafficType::Multicast) => MULTICAST,
            (false, false, TrafficType::Broadcast) => BROADCAST,
            (false, false, TrafficType::Unicast) => UNKNOWN,
        },
    )))
    .style(SvgType::AdaptColor)
    .width(Length::Fixed(FLAGS_WIDTH_BIG))
    .height(Length::Fixed(FLAGS_WIDTH_BIG * 0.75));

    let tooltip = match (is_my_address, is_local, traffic_type) {
        (true, _, _) => your_network_adapter_translation(language),
        (false, true, _) => local_translation(language),
        (false, false, TrafficType::Multicast) => "Multicast".to_string(),
        (false, false, TrafficType::Broadcast) => "Broadcast".to_string(),
        (false, false, TrafficType::Unicast) => unknown_translation(language),
    };

    Tooltip::new(content, tooltip, Position::FollowCursor)
        .font(font)
        .snap_within_viewport(true)
        .style(ContainerType::Tooltip)
}
