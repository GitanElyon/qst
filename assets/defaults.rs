{
AppConfig {
    general: GeneralConfig {
        rounded_corners: true,
        show_borders: true,
        highlight_symbol: Some(String::from(">> ")),
        favorite_symbol: Some(String::from("★ ")),
        favorite_key: Some(String::from("alt+f")),
        jump_to_top_key: Some(String::from("alt+up")),
        jump_to_bottom_key: Some(String::from("alt+down")),
        clipboard_command: None,
        log_level: None,
    },
    features: FeaturesConfig {
        enable_file_explorer: true,
        enable_launch_args: true,
        enable_auto_complete: true,
        dirs_first: true,
        show_duplicates: false,
        recent_first: true,
    },
    window: SectionConfig {
        title: None,
        fg: vec![],
        bg: vec![String::from("#000000")],
        border_color: vec![],
        border_angle: 90,
        rounded: None,
        borders: None,
        visible: Some(false),
        title_alignment: None,
        ..SectionConfig::default()
    },
    outer_box: SectionConfig {
        title: Some(String::from(" qst ")),
        fg: vec![],
        bg: vec![],
        border_color: vec![String::from("#cdd6f4")],
        border_angle: 90,
        rounded: None,
        borders: None,
        visible: Some(false),
        title_alignment: None,
        ..SectionConfig::default()
    },
    qst_ascii: QstAsciiConfig {
        section: SectionConfig {
            visible: Some(true),
            fg: vec![],
            ..SectionConfig::default()
        },
        gradient_colors: vec![String::from("#6464ff"), String::from("#c864ff")],
        gradient_angle: 90,
        alignment: Some(TextAlignment::Center),
        padding: PaddingConfig {
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        },
        custom_path: None,
    },
    input: SectionConfig {
        title: Some(String::from(" Search ")),
        fg: vec![],
        bg: vec![],
        border_color: vec![String::from("#6464ff")],
        border_angle: 90,
        rounded: None,
        borders: None,
        visible: None,
        title_alignment: None,
        ..SectionConfig::default()
    },
    list: ResultsConfig {
        section: SectionConfig {
            title: None,
            fg: vec![],
            bg: vec![],
            border_color: vec![String::from("#c864ff")],
            border_angle: 90,
            rounded: None,
            borders: None,
            visible: None,
            title_alignment: None,
            ..SectionConfig::default()
        },
        apps_title: None,
        files_title: None,
    },
    entry: EntryConfig {
        fg: vec![],
        bg: vec![],
        gradient_angle: 90,
    },
    entry_selected: SectionConfig {
        fg: vec![String::from("#111111")],
        bg: vec![String::from("#888888")],
        full_width_highlight: Some(true),
        ..SectionConfig::default()
    },
    meta: MetaConfig {
        active: SectionConfig {
            fg: vec![],
            bg: vec![String::from("#555555")],
            ..SectionConfig::default()
        },
        urgent: SectionConfig {
            fg: vec![String::from("red")],
            bg: vec![],
            ..SectionConfig::default()
        },
    },
    text: TextConfig {
        section: SectionConfig {
            fg: vec![String::from("#f2f5f7")],
            bg: vec![],
            visible: None,
            ..SectionConfig::default()
        },
        alignment: Some(TextAlignment::Left),
    },
}
}
