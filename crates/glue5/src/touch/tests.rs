use super::*;

/// An iPhone 13 Pro in landscape: 844 x 390 points, the notch's inset on
/// the left, the home indicator's at the bottom.
fn phone() -> Touch {
    let mut t = Touch::new();
    let screen = vec2(844.0, 390.0);
    t.layout(screen, Rect::from_min_max(pos2(47.0, 0.0), pos2(797.0, 369.0)), 1.0);
    t
}

/// The screens the layouts must fit: an iPhone 16 Pro, an iPhone 13 Pro, an
/// iPad Pro 11" (points, and safe insets left, top, right, bottom).
const SCREENS: [(Vec2, [f32; 4]); 3] = [
    (vec2(874.0, 402.0), [62.0, 0.0, 62.0, 21.0]),
    (vec2(844.0, 390.0), [47.0, 0.0, 47.0, 21.0]),
    (vec2(1194.0, 834.0), [0.0, 24.0, 0.0, 20.0]),
];

fn at(t: &Touch, p: Pos2) -> (f32, f32) {
    (p.x / t.screen.x, p.y / t.screen.y)
}

fn context(activity: Activity) -> PlayerContext {
    PlayerContext {
        activity,
        vehicle_near: true,
        phone: false,
        menu: false,
        cutscene: false,
        bar: false,
        scripted: false,
        character: Character::Franklin,
        gun: false,
        cursor: false,
        map: false,
        pointer: false,
        alert: false,
        hidden: false,
    }
}

/// Where the layout's button labelled `label` is now.
fn spot(t: &Touch, label: &str) -> Pos2 {
    t.buttons()
        .iter()
        .find(|b| matches!(b.key, Key::Action(a) if unsafe { (*a).label } == label))
        .map(|b| b.at)
        .unwrap_or_else(|| panic!("no {label}"))
}

const ACTIVITIES: [Activity; 8] = [
    Activity::OnFoot,
    Activity::Driving(Driving::Car),
    Activity::Driving(Driving::Bicycle),
    Activity::Driving(Driving::Heli),
    Activity::Driving(Driving::Plane),
    Activity::Driving(Driving::Submarine),
    Activity::Passenger,
    Activity::Parachute,
];

/// Where the phone shows in the game at its usual place, on a screen this
/// wide over high (as `glue5_runtime::input::phone_rect` has it).
fn phone_at(aspect: f32) -> [f32; 4] {
    let depth = 113.0 * 35f32.to_radians().tan();
    let u = 0.5 + 0.5 * 93.05 / (depth * 16.0 / 9.0);
    let v = 0.5 + 0.5 * 51.7 / depth;
    let (w, h) = (0.12 / aspect, 0.2417);
    [u - w, v - h, u + w, v + h]
}

#[test]
fn locked_on_a_drag_pushes_the_right_stick() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    // Aim held, its thumb trembling: no turn, a stick in the dead zone.
    let p = spot(&t, "Aim");
    let (x, y) = at(&t, p);
    t.finger_down(1, x, y);
    t.aim_locked = true;
    let (x, y) = at(&t, p + vec2(1.0, 0.0));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert!(pad.right[0] < 0.2 && pad.l2 == 1.0);
    assert_eq!(t.take_look(), (0, 0));
    // A swipe on the camera: the stick all the way, then easing off.
    let c = pos2(560.0, 160.0);
    let (x, y) = at(&t, c);
    t.finger_down(2, x, y);
    let (x, y) = at(&t, c + vec2(-2.0 * LOCKED_SWIPE, 0.0));
    t.finger_motion(2, x, y);
    assert_eq!(t.gamepad().right, [-1.0, 0.0]);
    assert_eq!(t.gamepad().right, [-1.0, 0.0]);
    assert!(t.gamepad().right[0] > -1.0);
    assert_eq!(t.take_look(), (0, 0));
    // Unlocked, drags turn the camera again.
    t.aim_locked = false;
    let (x, y) = at(&t, c);
    t.finger_motion(2, x, y);
    assert_eq!(t.gamepad().right, [0.0, 0.0]);
    assert_ne!(t.take_look(), (0, 0));
}

#[test]
fn a_tap_lifting_before_the_pad_is_read_presses_once() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    let (x, y) = at(&t, spot(&t, "Jump"));
    t.finger_down(4, x, y);
    t.finger_up(4);
    assert_eq!(t.gamepad().buttons, button::WEST);
    assert_eq!(t.gamepad().buttons, 0);
}

#[test]
fn touch_layouts_fit_without_overlapping() {
    for fire in [TouchFire::Auto, TouchFire::Manual] {
        for steer in [TouchSteer::Stick, TouchSteer::Arrows] {
            for (screen, [l, top, r, b]) in SCREENS {
                for profile in [TouchLayout::Classic, TouchLayout::Modern, TouchLayout::FiveStar] {
                    let mut t = Touch::new();
                    t.set_profile(profile);
                    t.set_steer(steer);
                    t.set_fire(fire);
                    let safe = Rect::from_min_max(pos2(l, top), pos2(screen.x - r, screen.y - b));
                    t.layout(screen, safe, 1.0);
                    let cases =
                        ACTIVITIES.into_iter().flat_map(|a| [(a, false, false, false), (a, true, false, false)]);
                    let extra = [(Activity::OnFoot, false, true, false), (Activity::OnFoot, false, false, true)];
                    for (activity, phone, cutscene, scripted) in cases.chain(extra) {
                        let check_sticky =
                            fire == TouchFire::Manual && activity == Activity::OnFoot && !cutscene && !scripted;
                        for aim_sticky in [false, true] {
                            if aim_sticky && !check_sticky {
                                continue;
                            }
                            t.set_context(PlayerContext { phone, cutscene, scripted, ..context(activity) });
                            t.aim_sticky = aim_sticky;
                            t.set_game(Vec::new(), phone.then(|| phone_at(screen.x / screen.y)));
                            let what = format!(
                                "{screen:?} {profile:?} {steer:?} {fire:?} {activity:?} phone {phone} cutscene \
                                 {cutscene} scripted {scripted} sticky {aim_sticky}"
                            );
                            for button in t.buttons() {
                                assert!(
                                    t.area.contains_rect(button.rect()),
                                    "{what}: {:?} hit target leaves safe area",
                                    button.key
                                );
                            }
                            let ring = if t.sprints() { SPRINT_REACH } else { 1.0 };
                            let mut all: Vec<(String, Pos2, f32)> =
                                t.buttons().iter().map(|b| (format!("{:?}", b.key), b.at, b.radius)).collect();
                            if t.stick_shown() {
                                all.push(("stick".into(), t.pos(MOVE_REST), ring * t.stick_radius()));
                            }
                            // The app's settings button, beside pause and the view.
                            all.push((
                                "settings".into(),
                                crate::ui::gear_center(safe, t.scale),
                                crate::ui::GEAR_RADIUS * t.scale,
                            ));
                            for (i, (a, pa, ra)) in all.iter().enumerate() {
                                let r = Rect::from_center_size(*pa, Vec2::splat(2.0 * ra));
                                assert!(t.area.expand(1.0).contains_rect(r), "{what}: {a} leaves the safe area");
                                for (b, pb, rb) in &all[i + 1..] {
                                    assert!(pa.distance(*pb) > ra + rb, "{what}: {a} overlaps {b}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn touch_targets_keep_a_48_point_minimum_and_fit_the_safe_area() {
    for (screen, [l, top, r, b]) in SCREENS {
        let area = Rect::from_min_max(pos2(l, top), pos2(screen.x - r, screen.y - b));
        for size in [0.7, 1.0, 1.5] {
            for profile in [TouchLayout::Classic, TouchLayout::Modern, TouchLayout::FiveStar] {
                for steer in [TouchSteer::Stick, TouchSteer::Arrows] {
                    for fire in [TouchFire::Auto, TouchFire::Manual] {
                        for activity in ACTIVITIES {
                            let mut t = Touch::new();
                            t.set_profile(profile);
                            t.set_steer(steer);
                            t.set_fire(fire);
                            t.layout(screen, area, size);
                            t.set_context(context(activity));
                            for button in t.buttons() {
                                assert!(button.hit_radius() >= 24.0, "{screen:?} {size} {profile:?} {activity:?}");
                                assert!(
                                    area.contains_rect(button.rect()),
                                    "{screen:?} {size} {profile:?} {activity:?}: {:?}",
                                    button.key
                                );
                            }
                        }
                    }
                }
            }
            assert!(MENU_KEYS.iter().all(|(_, key)| menu_key_hit_radius(key.radius * size) >= 24.0));
        }
    }
}

#[test]
fn overlapping_hit_areas_choose_the_button_nearest_the_finger() {
    let buttons = [
        Button {
            key: Key::Action(&PAUSE),
            at: pos2(100.0, 100.0),
            radius: 24.0,
            icon: "pause",
            bits: button::START,
            input: None,
            kind: Kind::Press,
            primary: false,
            asked: false,
        },
        Button {
            key: Key::Action(&VIEW),
            at: pos2(150.0, 100.0),
            radius: 24.0,
            icon: "view",
            bits: button::SELECT,
            input: None,
            kind: Kind::Press,
            primary: false,
            asked: false,
        },
    ];

    // Both targets contain this tap; the right-hand target is closer even
    // though it appears later in the layout list.
    let picked = nearest_hit_button(&buttons, pos2(127.0, 100.0)).unwrap();
    assert!(matches!(picked.key, Key::Action(action) if action == (&VIEW as *const Action)));
    assert!(buttons.iter().all(|button| button.rect().contains(pos2(127.0, 100.0))));
}

#[test]
fn arrows_steer_a_car_and_slide_between() {
    let mut t = phone();
    t.set_steer(TouchSteer::Arrows);
    t.set_context(context(Activity::Driving(Driving::Car)));
    let left = spot(&t, "◀");
    let right = spot(&t, "▶");
    assert!(!t.stick_shown());

    let (x, y) = at(&t, left);
    t.finger_down(1, x, y);
    assert_eq!(t.gamepad().left[0], -1.0);
    let (x, y) = at(&t, right);
    t.finger_motion(1, x, y);
    assert_eq!(t.gamepad().left[0], 1.0);

    let (x, y) = at(&t, left);
    t.finger_down(2, x, y);
    assert_eq!(t.gamepad().left[0], 0.0);
    t.finger_up(1);
    t.finger_up(2);
    assert_eq!(t.gamepad().left[0], 0.0);

    assert!(!matches!(t.touch_finger(pos2(320.0, 160.0)), Finger::Move { .. }));
}

#[test]
fn arrows_keep_the_stick_in_the_air() {
    let mut t = phone();
    t.set_steer(TouchSteer::Arrows);
    t.set_context(context(Activity::Driving(Driving::Heli)));
    assert!(!t.buttons().iter().any(|b| matches!(b.key, Key::Action(a) if unsafe { (*a).label } == "◀")));
    assert!(matches!(t.touch_finger(pos2(320.0, 160.0)), Finger::Move { .. }));
}

#[test]
fn auto_fire_aims_with_a_gun() {
    let mut t = phone();
    t.set_context(PlayerContext { gun: true, ..context(Activity::OnFoot) });
    let (x, y) = at(&t, spot(&t, "Fire"));
    t.finger_down(1, x, y);
    let pad = t.gamepad();
    assert_eq!(pad.buttons & (button::L2 | button::R2), button::L2 | button::R2);
    assert_eq!((pad.l2, pad.r2), (1.0, 1.0));

    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    let (x, y) = at(&t, spot(&t, "Fire"));
    t.finger_down(1, x, y);
    let pad = t.gamepad();
    assert_eq!(pad.buttons & button::L2, 0);
    assert_eq!(pad.l2, 0.0);
}

#[test]
fn the_support_trace_records_effective_fire_and_aim_output_without_coordinates() {
    let mut t = phone();
    t.set_context(PlayerContext { gun: true, ..context(Activity::OnFoot) });
    let (x, y) = at(&t, spot(&t, "Fire"));
    t.finger_down(1, x, y);
    let _ = t.gamepad();
    t.finger_up(1);
    let _ = t.gamepad();

    let trace = t.diagnostics();
    assert!(trace.iter().any(|line| line.contains("buttons=[L2,R2]") && line.contains("triggers=(1.00,1.00)")));
    assert!(trace.iter().any(|line| line.contains("buttons=[none]")));
    assert!(trace.iter().all(|line| !line.contains("x=") && !line.contains("y=")));
}

#[test]
fn manual_aim_shows_a_fire_on_the_left() {
    let mut t = phone();
    t.set_fire(TouchFire::Manual);
    t.set_context(context(Activity::OnFoot));
    let buttons = t.buttons();
    assert!(!buttons.iter().any(|b| {
        b.at.x < t.screen.x / 2.0 && matches!(b.key, Key::Action(a) if unsafe { (*a).label } == "Fire")
    }));

    let (x, y) = at(&t, spot(&t, "Aim"));
    t.finger_down(1, x, y);
    let buttons = t.buttons();
    let fire = buttons
        .iter()
        .find(|b| b.at.x < t.screen.x / 2.0 && matches!(b.key, Key::Action(a) if unsafe { (*a).label } == "Fire"))
        .expect("the left Fire button appears while Aim is held");
    let fire_at = fire.at;
    assert_eq!(fire.bits, button::R2);
    assert_eq!(fire.kind, Kind::Press);
    let (x, y) = at(&t, fire_at);
    t.finger_down(2, x, y);
    let pad = t.gamepad();
    assert_eq!(pad.buttons & (button::L2 | button::R2), button::L2 | button::R2);
    assert_eq!((pad.l2, pad.r2), (1.0, 1.0));

    t.finger_up(1);
    assert!(!t.buttons().iter().any(|b| {
        b.at.x < t.screen.x / 2.0 && matches!(b.key, Key::Action(a) if unsafe { (*a).label } == "Fire")
    }));

    let mut t = phone();
    t.set_fire(TouchFire::Manual);
    t.set_context(PlayerContext { gun: true, ..context(Activity::OnFoot) });
    let (x, y) = at(&t, spot(&t, "Fire"));
    t.finger_down(3, x, y);
    let pad = t.gamepad();
    assert_eq!(pad.buttons & button::L2, 0);
    assert_eq!(pad.l2, 0.0);
}

#[test]
fn a_double_tap_on_aim_keeps_aiming() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    for id in [1, 2] {
        let (x, y) = at(&t, spot(&t, "Aim"));
        t.finger_down(id, x, y);
        t.finger_up(id);
        if id == 1 {
            assert_eq!(t.gamepad().l2, 0.0);
        }
    }
    assert_eq!(t.gamepad().l2, 1.0);

    let (x, y) = at(&t, spot(&t, "Aim"));
    t.finger_down(3, x, y);
    t.finger_up(3);
    assert_eq!(t.gamepad().l2, 0.0);
}

#[test]
fn only_one_finger_turns_the_camera() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    let camera = pos2(520.0, 160.0);
    let aim = spot(&t, "Aim");
    let (x, y) = at(&t, camera);
    t.finger_down(1, x, y);
    let (x, y) = at(&t, aim);
    t.finger_down(2, x, y);

    let (x, y) = at(&t, camera + vec2(3.0, 0.0));
    t.finger_motion(1, x, y);
    let (x, y) = at(&t, aim + vec2(4.0, 0.0));
    t.finger_motion(2, x, y);
    assert_eq!(t.take_look(), ((3.0 * LOOK_GAIN) as i32, 0));

    t.finger_up(1);
    let (x, y) = at(&t, aim + vec2(7.0, 0.0));
    t.finger_motion(2, x, y);
    assert_eq!(t.take_look(), ((3.0 * LOOK_GAIN) as i32, 0));
}

#[test]
fn the_layout_follows_the_player() {
    let mut t = phone();
    let layout = |t: &Touch| t.touch_layout().map(|l| l[0].label);
    t.set_context(context(Activity::OnFoot));
    assert_eq!(layout(&t), Some("Fire"));
    t.set_context(context(Activity::Driving(Driving::Bicycle)));
    assert_eq!(layout(&t), Some("Pedal"));
    t.set_context(context(Activity::Parachute));
    assert_eq!(layout(&t), Some("Open"));
    t.set_context(PlayerContext { cutscene: true, ..context(Activity::Passenger) });
    assert_eq!(layout(&t), Some("Skip"));
    assert!(!t.has_stick());
    // A script's menu with its buttons' bar: the D-pad, Select, Back.
    t.set_context(PlayerContext { scripted: true, bar: true, ..context(Activity::OnFoot) });
    assert_eq!(layout(&t), Some("▲"));
    let bits: Vec<u16> = t.buttons().iter().map(|b| b.bits).collect();
    assert!(bits.contains(&button::SOUTH) && bits.contains(&button::EAST) && bits.contains(&button::LEFT));
    // One that asks for nothing (a death, a switch, a scene): pause alone,
    // no stick.
    t.set_context(PlayerContext { scripted: true, ..context(Activity::OnFoot) });
    assert_eq!(t.buttons().iter().map(|b| b.icon).collect::<Vec<_>>(), ["pause"]);
    assert!(!t.has_stick());
    // The phone's keys join the layout, the buttons the phone uses go.
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    let bits: Vec<u16> = t.buttons().iter().map(|b| b.bits).collect();
    for b in [button::UP, button::DOWN, button::LEFT, button::RIGHT, button::SOUTH, button::EAST] {
        assert!(bits.contains(&b), "no phone key {b:#x}");
    }
    for b in [button::WEST, button::NORTH, button::R2, button::L1] {
        assert!(!bits.contains(&b), "{b:#x} still shown with the phone out");
    }
    // In a car the pedals stay.
    t.set_context(PlayerContext { phone: true, ..context(Activity::Driving(Driving::Car)) });
    let bits: Vec<u16> = t.buttons().iter().map(|b| b.bits).collect();
    assert!(bits.contains(&button::R2) && bits.contains(&button::L2) && !bits.contains(&button::NORTH));
    // Enter only next to a vehicle.
    t.set_context(PlayerContext { vehicle_near: false, ..context(Activity::OnFoot) });
    assert!(t.buttons().iter().all(|b| b.bits != button::NORTH));
    // The pause menu: no layout, nor pad.
    t.set_context(PlayerContext { menu: true, ..context(Activity::OnFoot) });
    assert_eq!(layout(&t), None);
    assert_eq!(t.gamepad().buttons, 0);
}

#[test]
fn modern_layout_keeps_rare_actions_on_more() {
    let mut t = phone();
    t.set_profile(TouchLayout::Modern);
    t.set_context(context(Activity::OnFoot));
    let labels = |t: &Touch| -> Vec<&str> {
        t.buttons().iter().filter_map(|b| match b.key {
            Key::Action(a) => Some(unsafe { (*a).label }),
            _ => None,
        }).collect()
    };
    let modern = labels(&t);
    assert!(modern.contains(&"More"));
    assert!(!modern.contains(&"Special") && !modern.contains(&"Crouch") && !modern.contains(&"Throw"));
    assert!(modern.contains(&"Weapons") && modern.contains(&"Aim") && modern.contains(&"Fire"));

    // The fan picks Crouch, which then presses L3.
    let origin = spot(&t, "More");
    let actions = t.more_actions().unwrap();
    let i = actions.iter().position(|a| a.label == "Crouch").unwrap();
    let (x, y) = at(&t, origin);
    t.finger_down(1, x, y);
    assert_eq!(t.gamepad().buttons, 0, "More itself presses nothing");
    let (x, y) = at(&t, t.radial_place(origin, actions.len(), i));
    t.finger_motion(1, x, y);
    assert_eq!(t.gamepad().buttons, button::L3);
    t.finger_up(1);
    assert_eq!(t.gamepad().buttons, 0, "a held fan pick is not pressed again when it lifts");

    // A tap that starts and ends between two reads still reaches the game.
    let (ox, oy) = at(&t, origin);
    t.finger_down(2, ox, oy);
    t.finger_motion(2, x, y);
    t.finger_up(2);
    assert_eq!(t.gamepad().buttons, button::L3, "a fan tap survives the lift");

    // The Classic layout keeps every action on screen.
    t.set_profile(TouchLayout::Classic);
    let classic = labels(&t);
    for label in ["Special", "Crouch", "Throw"] {
        assert!(classic.contains(&label), "Classic has no {label}");
    }
}

#[test]
fn the_car_fan_holds_the_rare_driving_controls() {
    let mut t = phone();
    t.set_profile(TouchLayout::Modern);
    t.set_context(context(Activity::Driving(Driving::Car)));
    let labels: Vec<&str> = t.buttons().iter().filter_map(|b| match b.key {
        Key::Action(a) => Some(unsafe { (*a).label }),
        _ => None,
    }).collect();
    assert!(labels.contains(&"Gas") && labels.contains(&"Brake") && labels.contains(&"More"));
    assert!(!labels.contains(&"Horn") && !labels.contains(&"Lights"));
    let actions = t.more_actions().unwrap();
    for label in ["Horn", "Lights", "Behind", "Special"] {
        assert!(actions.iter().any(|a| a.label == label), "no {label} in the car fan");
    }
}

#[test]
fn the_game_asks_and_a_button_shows_it() {
    let mut t = phone();
    t.set_context(PlayerContext { scripted: true, bar: true, ..context(Activity::OnFoot) });
    // The character creator's prompts: Quit (B), Random (X), Turn Head.
    let prompts = vec![
        Prompt { pads: vec![Pad::Buttons(button::EAST)], input: Some(202), label: "Quit".into() },
        Prompt { pads: vec![Pad::Buttons(button::WEST)], input: Some(203), label: "Random".into() },
        Prompt { pads: vec![Pad::Stick { right: true, x: true, y: true }], input: None, label: "Turn Head".into() },
    ];
    t.set_game(prompts, None);
    let buttons = t.buttons();
    // Back does Quit: ringed; Random has a button of its own, pressed as X.
    assert!(buttons.iter().find(|b| b.bits == button::EAST).unwrap().asked);
    let own: Vec<&Button> = buttons.iter().filter(|b| matches!(b.key, Key::Prompt { .. })).collect();
    assert_eq!(own.len(), 1, "the stick is no button");
    let random = own[0];
    assert_eq!((random.icon, random.bits), ("pad-x", button::WEST));
    let (x, y) = at(&t, random.at);
    t.finger_down(1, x, y);
    assert_eq!(t.gamepad().buttons, button::WEST);
    t.finger_up(1);
    // A drag on the right is the right stick.
    let c = pos2(500.0, 120.0);
    let (x, y) = at(&t, c);
    t.finger_down(2, x, y);
    let (x, y) = at(&t, c + vec2(1.5 * t.stick_radius(), 0.0));
    t.finger_motion(2, x, y);
    assert_eq!(t.gamepad().right, [1.0, 0.0]);
    assert_eq!(t.take_look(), (0, 0));
}

#[test]
fn a_prompt_on_foot_lights_the_button_that_does_it() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    // "Press D-pad right to talk": Action does it, ringed; nothing added.
    t.set_game(vec![Prompt { pads: vec![Pad::Buttons(button::RIGHT)], input: Some(51), label: String::new() }], None);
    let buttons = t.buttons();
    assert!(buttons.iter().find(|b| b.bits == button::RIGHT).unwrap().asked);
    assert!(buttons.iter().all(|b| !matches!(b.key, Key::Prompt { .. })));
    // "Press Y to get in" (the prologue's car), no vehicle seen near: Enter,
    // which the layout hid, shows for it, ringed.
    t.set_context(PlayerContext { vehicle_near: false, ..context(Activity::OnFoot) });
    t.set_game(vec![Prompt { pads: vec![Pad::Buttons(button::NORTH)], input: Some(204), label: String::new() }], None);
    let buttons = t.buttons();
    let enter = buttons.iter().find(|b| b.icon == "enter").expect("Enter shows");
    assert!(enter.asked && buttons.iter().all(|b| !matches!(b.key, Key::Prompt { .. })));
    // "Press B to focus on Lamar" in a car (no button does it): its own,
    // the camera's focus, pressed as B.
    t.set_context(context(Activity::Driving(Driving::Car)));
    t.set_game(vec![Prompt { pads: vec![Pad::Buttons(button::EAST)], input: Some(80), label: String::new() }], None);
    let own = t.buttons().into_iter().find(|b| matches!(b.key, Key::Prompt { .. })).unwrap();
    assert_eq!((own.bits, own.input, own.icon), (button::EAST, None, "cinematic"));
    let occupied = t.occupied();
    assert!(occupied.iter().filter(|r| r.intersects(own.rect())).count() == 1, "it is on a control");
    // A script's own input in its scene (the pad's symbol in the game's
    // words): that symbol.
    t.set_context(PlayerContext { scripted: true, ..context(Activity::OnFoot) });
    t.set_game(vec![Prompt { pads: vec![Pad::Buttons(button::NORTH)], input: Some(225), label: String::new() }], None);
    let own = t.buttons().into_iter().find(|b| matches!(b.key, Key::Prompt { .. })).unwrap();
    assert_eq!(own.icon, "pad-y");
}

#[test]
fn an_input_only_prompt_presses_the_input() {
    let mut t = phone();
    t.set_context(context(Activity::Driving(Driving::Car)));
    // A script's scene waits for its skip: a button, the input pressed.
    t.set_game(vec![Prompt { pads: Vec::new(), input: Some(SKIP_CUTSCENE), label: String::new() }], None);
    let skip = t.buttons().into_iter().find(|b| matches!(b.key, Key::Prompt { .. })).unwrap();
    assert_eq!(skip.icon, "skip");
    let (x, y) = at(&t, skip.at);
    t.finger_down(1, x, y);
    assert_eq!(t.take_inputs(), vec![SKIP_CUTSCENE]);
    assert_eq!(t.gamepad().buttons, 0);
    t.finger_up(1);
    assert!(t.take_inputs().is_empty());
    // The cutscene's own skip presses it too.
    t.set_game(Vec::new(), None);
    t.set_context(PlayerContext { cutscene: true, ..context(Activity::OnFoot) });
    let (x, y) = at(&t, spot(&t, "Skip"));
    t.finger_down(2, x, y);
    assert_eq!(t.take_inputs(), vec![SKIP_CUTSCENE]);
}

#[test]
fn in_the_pause_menu_fingers_are_the_mouse() {
    let mut t = phone();
    t.set_context(PlayerContext { menu: true, ..context(Activity::OnFoot) });
    // A tap: the pointer there, a click that lasts until the engine sees it.
    let (x, y) = at(&t, pos2(400.0, 200.0));
    t.finger_down(1, x, y);
    t.finger_up(1);
    let m = t.take_mouse();
    assert!(!m.down && m.keys == [false; 6], "the pointer moves first");
    assert!((m.at.unwrap().x - x).abs() < 1e-6);
    std::thread::sleep(CLICK_DELAY);
    assert!(t.take_mouse().down);
    // A second finger pinches: the wheel, no click.
    t.finger_down(1, x, y);
    let (px, py) = at(&t, pos2(400.0, 320.0));
    t.finger_down(2, px, py);
    let (px, py) = at(&t, pos2(400.0, 320.0 + 2.0 * PINCH_NOTCH));
    t.finger_motion(2, px, py);
    let m = t.take_mouse();
    assert_eq!((m.wheel, m.down), (2, false));
    // Two fingers dragged up together: the wheel scrolls down.
    t.clear();
    let (ax, ay) = at(&t, pos2(400.0, 300.0));
    t.finger_down(1, ax, ay);
    let (bx, by) = at(&t, pos2(500.0, 300.0));
    t.finger_down(2, bx, by);
    let up = 3.0 * SCROLL_NOTCH + GESTURE_CLEAR;
    for (id, x) in [(1, 400.0), (2, 500.0)] {
        for step in 1..=4 {
            let (x, y) = at(&t, pos2(x, 300.0 - up * step as f32 / 4.0));
            t.finger_motion(id, x, y);
        }
    }
    let m = t.take_mouse();
    assert_eq!((m.wheel, m.down), (-3, false));
    // The keys: held while the finger is, and a tap's long enough for the
    // engine to see.
    for (i, (key, a)) in MENU_KEYS.iter().enumerate() {
        t.clear();
        let (x, y) = at(&t, t.pos(a.at));
        t.finger_down(3, x, y);
        t.finger_up(3);
        let m = t.take_mouse();
        assert!(m.keys[i] && m.keys.iter().filter(|k| **k).count() == 1 && !m.down, "{key:?}");
        std::thread::sleep(MIN_PRESS);
        assert!(!t.take_mouse().keys[i]);
    }
}

#[test]
fn a_menu_takes_taps_and_drags() {
    let mut t = phone();
    t.set_context(PlayerContext { menu: true, cursor: true, ..context(Activity::OnFoot) });
    // A tap clicks as it lifts, where it landed (the pointer there first).
    let (x, y) = at(&t, pos2(400.0, 200.0));
    t.finger_down(1, x, y);
    let m = t.take_mouse();
    assert!(!m.down && (m.at.unwrap().y - y).abs() < 1e-6, "no click while it may be a drag");
    let (x1, y1) = at(&t, pos2(405.0, 203.0));
    t.finger_motion(1, x1, y1);
    std::thread::sleep(CLICK_DELAY);
    assert!(!t.take_mouse().down);
    t.finger_up(1);
    let m = t.take_mouse();
    assert!(m.down && (m.at.unwrap().x - x).abs() < 1e-6, "a tap's small slip keeps the pointer");
    std::thread::sleep(MIN_PRESS);
    assert!(!t.take_mouse().down);
    // That click only went into the menu's column: it clicks again, once.
    t.menu_went_deeper(Some(Instant::now()));
    std::thread::sleep(AGAIN_DELAY);
    assert!(t.take_mouse().down);
    std::thread::sleep(MIN_PRESS);
    t.menu_went_deeper(Some(Instant::now()));
    assert!(!t.take_mouse().down);
    // A drag down scrolls up (natural scrolling), the pointer stays, no click.
    t.finger_down(1, x, y);
    for step in 1..=4 {
        let (x, y) = at(&t, pos2(400.0, 200.0 + step as f32 * SCROLL_NOTCH));
        t.finger_motion(1, x, y);
    }
    t.finger_up(1);
    std::thread::sleep(CLICK_DELAY);
    let m = t.take_mouse();
    assert!(!m.down && (m.at.unwrap().y - y).abs() < 1e-6);
    assert!((3..=4).contains(&m.wheel), "{}", m.wheel);
    // Sideways: Left or Right once a step, released between.
    t.finger_down(1, x, y);
    let (x3, y3) = at(&t, pos2(400.0 + 2.5 * SWIPE_STEP, 204.0));
    t.finger_motion(1, x3, y3);
    t.finger_up(1);
    let mut presses = Vec::new();
    let mut was = [false; 6];
    let until = Instant::now() + 8 * MIN_PRESS;
    while Instant::now() < until {
        let m = t.take_mouse();
        assert!(!m.down && m.wheel == 0);
        for i in 0..6 {
            if m.keys[i] && !was[i] {
                presses.push(MENU_KEYS[i].0);
            }
        }
        was = m.keys;
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(presses, [MenuKey::Right, MenuKey::Right]);
    // A shorter one back: the last one's half step is gone.
    t.finger_down(1, x3, y3);
    let (x4, y4) = at(&t, pos2(400.0 + 1.4 * SWIPE_STEP, 204.0));
    t.finger_motion(1, x4, y4);
    t.finger_up(1);
    let m = t.take_mouse();
    assert!(m.keys[1] && !m.keys[2], "{:?}", m.keys);
    // On the map, a drag moves the map: the mouse held, the pointer along.
    t.set_context(PlayerContext { menu: true, cursor: true, map: true, ..context(Activity::OnFoot) });
    t.finger_down(1, x, y);
    let (x2, y2) = at(&t, pos2(460.0, 240.0));
    t.finger_motion(1, x2, y2);
    std::thread::sleep(MIN_PRESS);
    let m = t.take_mouse();
    assert!(m.down && (m.at.unwrap().x - x2).abs() < 1e-6 && m.wheel == 0);
    t.finger_up(1);
    assert!(!t.take_mouse().down);
    // Its keys stay beside it.
    assert_eq!(t.occupied().len(), MENU_KEYS.len());
}

#[test]
fn a_scripts_cursor_menu_drops_the_pad() {
    let mut t = phone();
    // A shop's menu (the player's controls taken) with its cursor: no
    // layout, the menu's keys; a finger is its mouse.
    t.set_context(PlayerContext { scripted: true, bar: true, cursor: true, ..context(Activity::OnFoot) });
    assert!(t.buttons().is_empty() && t.occupied().len() == MENU_KEYS.len());
    let (x, y) = at(&t, pos2(150.0, 120.0));
    t.finger_down(1, x, y);
    t.finger_up(1);
    std::thread::sleep(CLICK_DELAY);
    assert!(t.take_mouse().down && t.gamepad().buttons == 0);
    // Without its cursor (a scene with its bar, not a menu for the mouse):
    // the D-pad.
    t.set_context(PlayerContext { scripted: true, bar: true, ..context(Activity::OnFoot) });
    assert!(t.is(SCRIPTED));
    // The interaction menu over play: the stick and buttons stay, a finger
    // elsewhere is the menu's mouse, not the camera.
    t.set_context(PlayerContext { bar: true, cursor: true, ..context(Activity::OnFoot) });
    assert!(t.on_foot());
    let (sx, sy) = at(&t, t.pos(MOVE_REST));
    t.finger_down(2, sx, sy);
    let (sx, sy) = at(&t, t.pos(MOVE_REST) + vec2(30.0, 0.0));
    t.finger_motion(2, sx, sy);
    assert!(t.gamepad().left[0] > 0.3, "the stick walks");
    let (x, y) = at(&t, pos2(150.0, 80.0));
    t.finger_down(1, x, y);
    t.finger_up(1);
    std::thread::sleep(CLICK_DELAY);
    let m = t.take_mouse();
    assert!(m.down && (m.at.unwrap().x - x).abs() < 1e-6);
    assert_eq!(t.take_look(), (0, 0));
    // The stick pushed far walks there, but does not sprint: A would pick
    // in the menu.
    let (sx, sy) = at(&t, t.pos(MOVE_REST) + vec2(3.0 * t.stick_radius(), 0.0));
    t.finger_motion(2, sx, sy);
    assert_eq!(t.gamepad().buttons & button::SOUTH, 0);
    // A screen with its own pointer (the browser): all the screen, though
    // the player's controls are not taken; Back alone.
    t.set_context(PlayerContext { cursor: true, pointer: true, ..context(Activity::OnFoot) });
    assert!(t.buttons().is_empty() && t.occupied().len() == 1);
}

#[test]
fn a_skippable_scene_always_shows_its_skip() {
    let mut t = phone();
    // A cutscene while it takes the skip, the player's controls taken or
    // not, with a bar or not.
    for (scripted, bar) in [(false, false), (true, false), (true, true)] {
        t.set_context(PlayerContext { cutscene: true, scripted, bar, ..context(Activity::OnFoot) });
        assert!(t.buttons().iter().any(|b| b.icon == "skip"), "{scripted} {bar}");
    }
    // A script's scene that waits for the skip (no cutscene of the
    // engine's), with or without its bar.
    for bar in [false, true] {
        t.set_context(PlayerContext { scripted: true, bar, ..context(Activity::OnFoot) });
        t.set_game(vec![Prompt { pads: Vec::new(), input: Some(SKIP_CUTSCENE), label: String::new() }], None);
        assert!(t.buttons().iter().any(|b| b.icon == "skip"), "{bar}");
    }
}

#[test]
fn the_pause_menus_keys_keep_off_the_menu() {
    for (screen, [l, top, r, b]) in SCREENS {
        let mut t = Touch::new();
        let safe = Rect::from_min_max(pos2(l, top), pos2(screen.x - r, screen.y - b));
        t.layout(screen, safe, 1.0);
        // The menu: the middle 16:9 of the screen, less its margins (its
        // right edge at 0.778 of an iPhone's width).
        let width = screen.x.min(screen.y * 16.0 / 9.0);
        let right = screen.x / 2.0 + width * (0.778 - 0.5) * (874.0 / 715.0);
        for (key, a) in MENU_KEYS.iter().skip(1) {
            let c = t.pos(a.at);
            assert!(c.x - a.radius * t.scale >= right, "{screen:?} {key:?} on the menu");
            assert!(safe.contains_rect(Rect::from_center_size(c, Vec2::splat(2.0 * a.radius * t.scale))));
        }
    }
}

#[test]
fn with_the_phone_out_the_stick_does_not_sprint() {
    let mut t = phone();
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    let o = pos2(200.0, 250.0);
    let (x, y) = at(&t, o);
    t.finger_down(1, x, y);
    let (x, y) = at(&t, o + vec2(0.0, -2.0 * t.stick_radius()));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert_eq!((pad.left, pad.buttons & button::SOUTH), ([0.0, -1.0], 0));
}

#[test]
fn the_phone_keys_keep_off_the_phone() {
    for (screen, [l, top, r, b]) in SCREENS {
        let safe = Rect::from_min_max(pos2(l, top), pos2(screen.x - r, screen.y - b));
        for size in [0.7, 1.0, 1.5] {
            let mut t = Touch::new();
            t.layout(screen, safe, size);
            for activity in [Activity::OnFoot, Activity::Driving(Driving::Car)] {
                t.set_context(PlayerContext { phone: true, ..context(activity) });
                t.set_game(Vec::new(), Some(phone_at(screen.x / screen.y)));
                let phone = t.phone.unwrap();
                assert!(phone.right() <= screen.x, "{screen:?}: the phone is on the screen");
                for key in t.buttons().iter().filter(|b| matches!(b.key, Key::Phone(_))) {
                    let what = format!("{screen:?} size {size} {activity:?} {:?}", key.key);
                    assert!(!key.rect().intersects(phone), "{what} on the phone {phone:?}");
                    assert!(t.area.contains_rect(key.rect()), "{what} off the safe area");
                }
            }
        }
    }
}

#[test]
fn on_foot_fingers_walk_sprint_and_turn_the_camera() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    let r = t.stick_radius();
    // The stick where the left thumb lands; past its ring, a sprint.
    let o = pos2(200.0, 250.0);
    let (x, y) = at(&t, o);
    t.finger_down(1, x, y);
    let (x, y) = at(&t, o + vec2(0.0, -0.5 * r));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert!((pad.left[1] + 0.5).abs() < 1e-3 && pad.buttons & button::SOUTH == 0);
    let (x, y) = at(&t, o + vec2(0.0, -1.5 * r));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert_eq!((pad.left, pad.buttons & button::SOUTH), ([0.0, -1.0], button::SOUTH));
    // Dragging on the right turns the camera by as much, nothing else.
    let c = pos2(560.0, 160.0);
    let (x, y) = at(&t, c);
    t.finger_down(2, x, y);
    let (x, y) = at(&t, c + vec2(10.0, -5.0));
    t.finger_motion(2, x, y);
    assert_eq!(t.take_look(), ((10.0 * LOOK_GAIN) as i32, (-5.0 * LOOK_GAIN) as i32));
    assert_eq!(t.take_look(), (0, 0));
    assert_eq!(t.gamepad().right, [0.0, 0.0]);
    t.finger_up(2);
    // Fire held fires and turns the camera.
    let p = spot(&t, "Fire");
    let (x, y) = at(&t, p);
    t.finger_down(3, x, y);
    let (x, y) = at(&t, p + vec2(-4.0, 0.0));
    t.finger_motion(3, x, y);
    assert_eq!(t.gamepad().buttons & button::R2, button::R2);
    assert_eq!(t.take_look(), ((-4.0 * LOOK_GAIN) as i32, 0));
}

#[test]
fn a_wheel_is_picked_by_dragging_from_its_button() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    let p = spot(&t, "Weapons");
    let (x, y) = at(&t, p);
    t.finger_down(1, x, y);
    let (x, y) = at(&t, p + vec2(2.0 * t.stick_radius(), 0.0));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert_eq!((pad.buttons & button::L1, pad.right), (button::L1, [1.0, 0.0]));
    assert_eq!(t.take_look(), (0, 0));
}

#[test]
fn an_open_wheel_picks_with_the_other_thumb() {
    let mut t = phone();
    t.set_context(context(Activity::Driving(Driving::Car)));
    // A thumb already on the camera, then the radio held with the other.
    let c = pos2(560.0, 160.0);
    let (x, y) = at(&t, c);
    t.finger_down(1, x, y);
    let (x, y) = at(&t, c + vec2(5.0, 0.0));
    t.finger_motion(1, x, y);
    let (x, y) = at(&t, spot(&t, "Radio"));
    t.finger_down(2, x, y);
    t.take_look();
    // Its drag picks from where it was, the camera stays.
    let (x, y) = at(&t, c + vec2(5.0, -2.0 * t.stick_radius()));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert_eq!((pad.buttons & button::LEFT, pad.right), (button::LEFT, [0.0, -1.0]));
    assert_eq!(t.take_look(), (0, 0));
    // The radio let go: the camera again.
    t.finger_up(2);
    let (x, y) = at(&t, c + vec2(15.0, -2.0 * t.stick_radius()));
    t.finger_motion(1, x, y);
    assert_eq!(t.gamepad().right, [0.0, 0.0]);
    assert_eq!(t.take_look(), ((10.0 * LOOK_GAIN) as i32, 0));
}

#[test]
fn an_open_wheel_picks_with_the_thumb_on_the_gas() {
    let mut t = phone();
    t.set_context(context(Activity::Driving(Driving::Car)));
    let g = spot(&t, "Gas");
    let (x, y) = at(&t, g);
    t.finger_down(1, x, y);
    let (x, y) = at(&t, g + vec2(3.0, 0.0));
    t.finger_motion(1, x, y);
    let (x, y) = at(&t, spot(&t, "Radio"));
    t.finger_down(2, x, y);
    // The thumb on the gas picks from where it was, still pressing it.
    let (x, y) = at(&t, g + vec2(3.0 - 2.0 * t.stick_radius(), 0.0));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert_eq!(pad.buttons & (button::LEFT | button::R2), button::LEFT | button::R2);
    assert_eq!(pad.right, [-1.0, 0.0]);
    assert_eq!(t.take_look(), (0, 0));
    // The radio let go: the gas only.
    t.finger_up(2);
    assert_eq!(t.gamepad().right, [0.0, 0.0]);
}

#[test]
fn driving_has_its_pedals() {
    let mut t = phone();
    t.set_context(context(Activity::Driving(Driving::Car)));
    for (id, label) in [(1, "Gas"), (2, "Brake")] {
        let (x, y) = at(&t, spot(&t, label));
        t.finger_down(id, x, y);
    }
    let pad = t.gamepad();
    assert_eq!((pad.r2, pad.l2), (1.0, 1.0));
    // Steering, without a sprint.
    let (x, y) = at(&t, pos2(200.0, 250.0));
    t.finger_down(3, x, y);
    let (x, y) = at(&t, pos2(200.0 - 2.0 * t.stick_radius(), 250.0));
    t.finger_motion(3, x, y);
    let pad = t.gamepad();
    assert_eq!((pad.left, pad.buttons & button::SOUTH), ([-1.0, 0.0], 0));
}

#[test]
fn every_icon_loads() {
    let ctx = egui::Context::default();
    let icons = load_icons(&ctx);
    assert_eq!(icons.len(), ICON_FILES.len(), "an icon does not decode");
    let layouts = [ON_FOOT, CAR, BICYCLE, HELI, PLANE, SUBMARINE, PASSENGER, PARACHUTE, CUTSCENE, SCRIPTED, &PHONE_KEYS];
    let names = layouts.iter().flat_map(|l| l.iter().flat_map(|a| a.icon.split('+')));
    let pads = (0..16).map(|i| pad_icon(1 << i, None)).chain([pad_icon(0, Some(SKIP_CUTSCENE))]);
    for name in names.chain(MENU_KEYS.iter().map(|(_, a)| a.icon)).chain(pads).chain(["reload", "settings"]) {
        assert!(icons.contains_key(name), "no icon {name}");
    }
}

#[test]
fn a_thumb_drifting_off_the_axis_keeps_its_reach() {
    let at = |deg: f32, reach: f32| stick_value(Vec2::angled(deg.to_radians()) * reach);
    let near = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4;
    // Sideways and 30 degrees up, all the way: full steering.
    assert!(near(at(-150.0, 2.0), [-1.0, -(30f32.to_radians().tan())]));
    // Half way: half.
    assert!((at(-30.0, 0.5)[0] - 0.5).abs() < 1e-4);
    assert!(near(at(90.0, 1.0), [0.0, 1.0]));
    assert!(near(at(45.0, 1.0), [1.0, 1.0]));
    assert_eq!(stick_value(Vec2::ZERO), [0.0, 0.0]);
}

#[test]
fn in_a_car_the_radio_wheel_picks_with_the_other_thumb() {
    let mut t = phone();
    t.set_context(context(Activity::Driving(Driving::Car)));
    let radio = spot(&t, "Radio");
    let (x, y) = at(&t, radio);
    t.finger_down(1, x, y);
    // The right thumb, on the gas or anywhere free, dragged right.
    for (id, from) in [(2, spot(&t, "Gas")), (3, pos2(560.0, 120.0))] {
        let (x, y) = at(&t, from);
        t.finger_down(id, x, y);
        // A short drag: the wheel takes the stick from 60 % on.
        let (x, y) = at(&t, from + vec2(2.0 * WHEEL_PICK, 0.0));
        t.finger_motion(id, x, y);
        let pad = t.gamepad();
        assert!(pad.buttons & button::LEFT != 0, "the radio's button is held");
        assert!(pad.right[0] > 0.99, "{id}: the wheel picks to the right: {:?}", pad.right);
        t.finger_up(id);
    }
}

#[test]
fn in_first_person_the_buttons_press_where_the_game_maps_them() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    // First person (standard_fps.meta): sprint on L3, duck on A.
    t.set_pad_map(vec![(SPRINT, button::L3), (36, button::SOUTH), (22, button::WEST)]);
    let o = t.pos(MOVE_REST);
    let (x, y) = at(&t, o);
    t.finger_down(1, x, y);
    let (x, y) = at(&t, o + vec2(0.0, -2.0 * t.stick_radius()));
    t.finger_motion(1, x, y);
    let pad = t.gamepad();
    assert_eq!(pad.buttons & (button::L3 | button::SOUTH), button::L3, "the stick sprints with L3, does not duck");
    t.finger_up(1);
    let (x, y) = at(&t, spot(&t, "Crouch"));
    t.finger_down(2, x, y);
    assert_eq!(t.gamepad().buttons, button::SOUTH, "Crouch presses A");
    t.finger_up(2);
    // Third person: the layouts' own buttons.
    t.set_pad_map(Vec::new());
    t.finger_down(3, x, y);
    assert_eq!(t.gamepad().buttons, button::L3);
}

#[test]
fn an_engine_warning_shows_its_buttons_alone() {
    let mut t = phone();
    // Driving, a pedal held, the game asks "Are you sure you want to join a
    // different online session?" with Yes (A) and No (B): the pedal lets go.
    t.set_context(context(Activity::Driving(Driving::Car)));
    let pedal = t.buttons().into_iter().find(|b| b.icon == "gas").expect("gas");
    let (x, y) = at(&t, pedal.at);
    t.finger_down(9, x, y);
    assert!(t.gamepad().r2 > 0.0);
    t.set_context(PlayerContext { alert: true, ..context(Activity::Driving(Driving::Car)) });
    assert_eq!((t.gamepad().r2, t.gamepad().buttons), (0.0, 0));
    let prompts = vec![
        Prompt { pads: vec![Pad::Buttons(button::SOUTH)], input: Some(201), label: "Yes".into() },
        Prompt { pads: vec![Pad::Buttons(button::EAST)], input: Some(202), label: "No".into() },
    ];
    t.set_game(prompts.clone(), None);
    let buttons = t.buttons();
    assert!(!t.has_stick());
    assert!(buttons.iter().all(|b| matches!(b.key, Key::Prompt { .. })), "no button of the car's");
    let bits: Vec<u16> = buttons.iter().map(|b| b.bits).collect();
    assert_eq!(bits, [button::SOUTH, button::EAST]);
    let (x, y) = at(&t, buttons[0].at);
    t.finger_down(1, x, y);
    assert_eq!(t.gamepad().buttons, button::SOUTH);
    t.finger_up(1);
    // Over the pause menu too: its buttons, not the menu's mouse.
    t.set_context(PlayerContext { alert: true, menu: true, cursor: true, ..context(Activity::OnFoot) });
    t.set_game(prompts, None);
    assert!(!t.menu_mode());
    assert_eq!(t.buttons().len(), 2);
    // "You were kicked from the session", its buttons not read: Select
    // and Back all the same.
    t.set_context(PlayerContext { alert: true, ..context(Activity::OnFoot) });
    t.set_game(Vec::new(), None);
    let bits: Vec<u16> = t.buttons().iter().map(|b| b.bits).collect();
    assert_eq!(bits, [button::SOUTH, button::EAST]);
    // Black, or loading: nothing of play's.
    t.set_context(PlayerContext { hidden: true, ..context(Activity::OnFoot) });
    t.set_game(Vec::new(), None);
    assert!(t.buttons().is_empty());
}

#[test]
fn drive_by_shoot_button_can_aim_while_held() {
    for layout in [TouchLayout::Classic, TouchLayout::Modern] {
        for aim_locked in [false, true] {
            let mut t = phone();
            t.set_profile(layout);
            t.aim_locked = aim_locked;
            t.set_context(context(Activity::Driving(Driving::Car)));
            let shoot = t
                .buttons()
                .into_iter()
                .find(|button| matches!(button.key, Key::Action(a) if unsafe { (*a).label == "Shoot" }))
                .expect("drive-by control");
            let center = shoot.at;
            let (x, y) = at(&t, center);
            t.finger_down(1, x, y);
            assert_eq!(t.gamepad().buttons & (button::L1 | button::R1), button::L1 | button::R1);

            let (x, y) = at(&t, center + vec2(6.0, 0.0));
            t.finger_motion(1, x, y);
            let pad = t.gamepad();
            if aim_locked {
                assert!(pad.right[0] > 0.5, "{layout:?}: locked aim should receive the drag");
            } else {
                assert_ne!(t.take_look(), (0, 0), "{layout:?}: dragging Shoot should aim the drive-by");
            }
            assert_eq!(pad.buttons & (button::L1 | button::R1), button::L1 | button::R1);
            t.finger_up(1);
        }
    }
}

#[test]
fn opening_phone_releases_the_held_open_button() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    let (x, y) = at(&t, spot(&t, "Phone"));
    t.finger_down(1, x, y);
    assert_ne!(t.gamepad().buttons & button::UP, 0);
    assert_eq!(t.gamepad().buttons & button::UP, 0, "the phone action is a press, not a held key");
    assert_eq!(t.gamepad().buttons & button::UP, 0, "a long touch must not toggle the phone again");
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    assert_eq!(t.gamepad().buttons & button::UP, 0, "opening finger must not repeat");
    assert_eq!(t.gamepad().buttons & button::UP, 0);
    t.finger_up(1);
    assert!(!t.take_mouse().down, "the opening finger is not a phone tap");
}

#[test]
fn opening_phone_releases_other_held_buttons_and_closing_releases_its_keys() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    // Melee is East — the phone's Cancel. Held over from play while the
    // phone opens, it would put the phone straight back away.
    let (x, y) = at(&t, spot(&t, "Melee\nReload"));
    t.finger_down(1, x, y);
    assert_eq!(t.gamepad().buttons & button::EAST, button::EAST);
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    assert_eq!(t.gamepad().buttons & button::EAST, 0, "a held Cancel must not survive the phone opening");
    assert_eq!(t.gamepad().buttons, 0);
    t.finger_up(1);

    // And the phone's own Back, still held as the phone goes away, must
    // not go on pressing in play.
    t.set_game(Vec::new(), Some(phone_at(t.screen.x / t.screen.y)));
    let back = t.buttons().iter().find(|b| matches!(b.key, Key::Phone(5))).map(|b| b.at).expect("the phone's Back key");
    let (x, y) = at(&t, back);
    t.finger_down(2, x, y);
    assert_eq!(t.gamepad().buttons & button::EAST, button::EAST);
    t.set_context(context(Activity::OnFoot));
    assert_eq!(t.gamepad().buttons & button::EAST, 0, "the phone's Back must not survive it closing");
    t.finger_up(2);
}

#[test]
fn the_phone_button_pulses_once_until_the_phone_is_seen_out() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    let (x, y) = at(&t, spot(&t, "Phone"));
    t.finger_down(1, x, y);
    assert_eq!(t.gamepad().buttons & button::UP, button::UP);
    t.finger_up(1);
    // The same tap registered again (a duplicate event, a second finger,
    // a retap while the phone is still coming up): no second pulse,
    // which would put the phone back down as it opens.
    t.finger_down(2, x, y);
    assert_eq!(t.gamepad().buttons & button::UP, 0, "a second pulse would close the opening phone");
    t.finger_up(2);
    // Seen out, then put away: it pulses again.
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    t.set_context(context(Activity::OnFoot));
    let (x, y) = at(&t, spot(&t, "Phone"));
    t.finger_down(3, x, y);
    assert_eq!(t.gamepad().buttons & button::UP, button::UP);
    t.finger_up(3);
    // Never seen out (the game did not open it): after the wait, a tap
    // may try again.
    t.phone_requested = Some(Instant::now() - (PHONE_RETRY + Duration::from_millis(1)));
    t.finger_down(4, x, y);
    assert_eq!(t.gamepad().buttons & button::UP, button::UP);
}

#[test]
fn with_the_phone_out_unplaced_a_tap_on_its_place_presses_no_button() {
    let mut t = phone();
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    // Its place is not known yet (set_game was never called): where the
    // phone is taken to be, a tap is on the phone, on no layout button
    // (Back there would put the phone that is opening away again).
    let (x, y) = at(&t, t.phone_area().center());
    t.finger_down(1, x, y);
    assert!(matches!(t.fingers.get(&1), Some(Finger::Phone { .. })));
    assert_eq!(t.gamepad().buttons, 0);
    t.finger_up(1);
    assert_eq!(t.gamepad().buttons, 0, "the tap must not put the phone away");
}

/// With the phone out, a tap clicks its actual screen coordinate instead of
/// accepting the app that happened to be highlighted; swipes remain available
/// for directional navigation and neither gesture moves or turns the player.
#[test]
fn the_phone_screen_takes_direct_taps_and_keeps_swipe_navigation() {
    let mut t = phone();
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    t.set_game(Vec::new(), Some(phone_at(t.screen.x / t.screen.y)));
    let c = t.phone.unwrap().center();
    let (x, y) = at(&t, c);
    t.finger_down(1, x, y);
    let pointer = t.take_mouse().at.unwrap();
    assert!(
        (pointer.x - x).abs() < 1.0e-6 && (pointer.y - y).abs() < 1.0e-6,
        "pointer follows the touched app"
    );
    t.finger_up(1);
    let click = t.take_mouse();
    assert!(click.down, "a tap clicks the touched point");
    assert_eq!(t.gamepad().buttons & button::SOUTH, 0, "do not launch the old selection");
    assert_eq!(t.gamepad().left, [0.0, 0.0]);

    t.finger_down(2, x, y);
    let (x2, y2) = at(&t, c + vec2(4.0, -PHONE_STEP - 1.0));
    t.finger_motion(2, x2, y2);
    let pad = t.gamepad();
    assert_eq!(pad.buttons & (button::UP | button::SOUTH), button::UP, "a swipe can still step up");
    let (x3, y3) = at(&t, c + vec2(PHONE_STEP + 6.0, -PHONE_STEP - 1.0));
    t.finger_motion(2, x3, y3);
    assert_eq!(t.gamepad().buttons & button::RIGHT, button::RIGHT, "then right");
    t.finger_up(2);
    assert!(!t.take_mouse().down, "a swipe is not an accidental click");
    assert_eq!(t.gamepad().buttons & button::SOUTH, 0, "a swipe does not select");
    assert_eq!(t.look, Vec2::ZERO, "nor turns the camera");
}

/// Put away, the phone's place is the camera's again.
#[test]
fn without_the_phone_its_place_turns_the_camera() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    t.set_game(Vec::new(), Some(phone_at(t.screen.x / t.screen.y)));
    let c = t.phone.map_or(pos2(700.0, 250.0), |r| r.center());
    let (x, y) = at(&t, c);
    t.finger_down(1, x, y);
    assert!(!matches!(t.fingers.get(&1), Some(Finger::Phone { .. })));
}

#[test]
fn five_star_aim_taps_toggle_and_flight_uses_a_second_stick() {
    let mut t = phone();
    t.set_profile(TouchLayout::FiveStar);
    t.set_context(context(Activity::OnFoot));
    let aim = spot(&t, "Aim");
    let (x, y) = at(&t, aim);
    t.finger_down(1, x, y);
    t.finger_up(1);
    assert!(t.is_aiming());
    assert_eq!(t.gamepad().l2, 1.0);
    t.finger_down(2, x, y);
    t.finger_up(2);
    assert!(!t.is_aiming());

    t.set_context(context(Activity::Driving(Driving::Heli)));
    assert!(t.flying_with_right_stick());
    let start = pos2(500.0, 150.0);
    let (x, y) = at(&t, start);
    t.finger_down(3, x, y);
    let (x, y) = at(&t, start + vec2(30.0, -30.0));
    t.finger_motion(3, x, y);
    let pad = t.gamepad();
    assert!(pad.right[0] > 0.0 && pad.right[1] < 0.0);
    assert!(pad.r2 > 0.0, "pulling up the helicopter flight stick should climb");
}

#[test]
fn five_star_pause_menu_adds_tab_buttons_without_changing_legacy_menu_keys() {
    let mut t = phone();
    t.set_profile(TouchLayout::FiveStar);
    t.set_context(PlayerContext { menu: true, ..context(Activity::OnFoot) });
    let occupied = t.occupied();
    assert_eq!(occupied.len(), FIVE_STAR_MENU_KEYS.len());
    for (i, rect) in occupied.iter().enumerate() {
        assert!(t.area.contains_rect(*rect), "Five Star menu key leaves the safe area: {rect:?}");
        for other in &occupied[i + 1..] {
            assert!(!rect.intersects(*other), "Five Star menu hit targets overlap: {rect:?} / {other:?}");
        }
    }

    for (key, expected) in [(MenuKey::TabLeft, button::L1), (MenuKey::TabRight, button::R1)] {
        let action = FIVE_STAR_MENU_KEYS.iter().find(|(candidate, _)| *candidate == key).unwrap().1;
        let p = t.menu_key_pos(&action);
        let (x, y) = at(&t, p);
        t.finger_down(1, x, y);
        assert_eq!(t.gamepad().buttons, expected);
        t.finger_up(1);
        assert_eq!(t.gamepad().buttons, 0);
    }

    t.set_profile(TouchLayout::Classic);
    assert_eq!(t.occupied().len(), MENU_KEYS.len());
}

fn debug_area(action: DebugTouchAction, when: DebugTouchWhen) -> DebugTouchArea {
    DebugTouchArea {
        name: "End call".to_owned(),
        rect: [0.4, 0.4, 0.6, 0.6],
        action,
        button: DebugTouchButton::Back,
        when,
    }
}

#[test]
fn debug_area_button_pulses_once_and_click_clicks_its_centre() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    t.set_debug(vec![debug_area(DebugTouchAction::PadButton, DebugTouchWhen::Always)], DebugAreas::Active);
    t.finger_down(1, 0.5, 0.5);
    assert_eq!(t.gamepad().buttons, button::EAST);
    assert_eq!(t.gamepad().buttons, 0, "a debug button pulses once, not held");
    t.finger_up(1);
    assert_eq!(t.gamepad().buttons, 0);

    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    t.set_debug(vec![debug_area(DebugTouchAction::Click, DebugTouchWhen::Always)], DebugAreas::Active);
    t.finger_down(1, 0.5, 0.5);
    assert!(!t.take_mouse().down, "a click area clicks as its tap lifts");
    t.finger_up(1);
    let mouse = t.take_mouse();
    assert_eq!(mouse.at, Some(pos2(0.5, 0.5)));
    assert!(mouse.down);

    // Dragging off an area before lifting is no tap on it.
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    t.set_debug(vec![debug_area(DebugTouchAction::Click, DebugTouchWhen::Always)], DebugAreas::Active);
    t.finger_down(1, 0.5, 0.5);
    t.finger_motion(1, 0.9, 0.5);
    t.finger_up(1);
    assert!(!t.take_mouse().down, "a finger lifted outside its area clicks nothing");
}

#[test]
fn debug_areas_show_only_on_their_screens() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    t.set_debug(vec![debug_area(DebugTouchAction::Click, DebugTouchWhen::Phone)], DebugAreas::Active);
    let centre = pos2(0.5 * t.screen.x, 0.5 * t.screen.y);
    assert_eq!(t.debug_at(centre), None, "a phone area stays out of gameplay");
    t.set_context(PlayerContext { phone: true, ..context(Activity::OnFoot) });
    assert_eq!(t.debug_at(centre), Some(0));
}

#[test]
fn editing_debug_areas_draws_moves_and_resizes_without_acting() {
    let mut t = phone();
    t.set_context(context(Activity::OnFoot));
    t.set_debug(Vec::new(), DebugAreas::Edit);
    let (x, y) = at(&t, pos2(169.0, 78.0));
    t.finger_down(1, x, y);
    let (x, y) = at(&t, pos2(422.0, 234.0));
    t.finger_motion(1, x, y);
    t.finger_up(1);
    assert_eq!(t.debug_areas().len(), 1);
    assert_eq!(t.gamepad().buttons, 0, "drawing an area presses nothing");
    assert!(!t.take_mouse().down, "drawing an area clicks nothing");
    for (got, want) in t.debug_areas()[0].rect.iter().zip([0.2, 0.2, 0.5, 0.6]) {
        assert!((got - want).abs() < 0.001, "drawn rect {:?}, want around {want}", t.debug_areas()[0].rect);
    }

    // A finger inside takes the whole area along. (Leave the editor first:
    // while it stands, the settings' copy does not overwrite its areas.)
    t.set_debug(vec![debug_area(DebugTouchAction::Click, DebugTouchWhen::Always)], DebugAreas::Active);
    let loaded = t.debug_areas().to_vec();
    t.set_debug(loaded, DebugAreas::Edit);
    let (x, y) = at(&t, pos2(422.0, 195.0));
    t.finger_down(2, x, y);
    let (x, y) = at(&t, pos2(464.0, 234.0));
    t.finger_motion(2, x, y);
    t.finger_up(2);
    for (got, want) in t.debug_areas()[0].rect.iter().zip([0.45, 0.5, 0.65, 0.7]) {
        assert!((got - want).abs() < 0.001, "moved rect {:?}, want around {want}", t.debug_areas()[0].rect);
    }

    // A finger on a corner stretches that corner.
    let rect = t.debug_rect(&t.debug_areas()[0]);
    let (x, y) = at(&t, rect.right_bottom());
    t.finger_down(3, x, y);
    let (x, y) = at(&t, pos2(591.0, 312.0));
    t.finger_motion(3, x, y);
    t.finger_up(3);
    for (got, want) in t.debug_areas()[0].rect.iter().zip([0.45, 0.5, 0.7, 0.8]) {
        assert!((got - want).abs() < 0.001, "resized rect {:?}, want around {want}", t.debug_areas()[0].rect);
    }

    // The caller's copy does not overwrite the editor's areas mid-edit,
    // and a stray tap draws no area that stays.
    let edited = t.debug_areas().to_vec();
    t.set_debug(Vec::new(), DebugAreas::Edit);
    assert_eq!(t.debug_areas(), edited.as_slice());
    t.set_debug(Vec::new(), DebugAreas::Active);
    t.set_debug(Vec::new(), DebugAreas::Edit);
    let (x, y) = at(&t, pos2(100.0, 100.0));
    t.finger_down(4, x, y);
    t.finger_up(4);
    assert!(t.debug_areas().is_empty(), "a tap too small to use is no area");
}
