use std::{cmp::min, collections::VecDeque, fs};

use bevy::{
    ecs::system::SystemState, input::{keyboard::KeyboardInput, ButtonState}, prelude::*, reflect::List, utils::dbg, winit::WinitSettings
};
use bevy_inspector_egui::quick::StateInspectorPlugin;
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin};

mod text_components;

use text_components::{scroll, AppWindow, Character, Document, DocumentPlugin, Line, Scroll, Span, WorkingFilePath};

#[derive(Component)]
pub struct MainCamera;

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum AppState {
    Normal,
    Insert,
    #[default]
    Travel,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "text editor".into(),
                name: Some("editor".into()),
                // present_mode: PresentMode::Immediate,
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin)
        .add_plugins(StateInspectorPlugin::<AppState>::default())
        .add_plugins(PerfUiPlugin)
        .add_plugins(DocumentPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, (setup, setup_root_zipper).chain())
        .add_systems(Update, (
            control_normal.run_if(in_state(AppState::Normal)),
            control_travel.run_if(in_state(AppState::Travel)),
            (control_insert, process_insert).run_if(in_state(AppState::Insert)),
            (move_char_left_right, move_char_up_down)
                .before(goto_char)
                .after(control_normal),
            highlight_border,
            dehighlight_border,
            despawn_zipper,
            move_zipper,
            goto_char.before(move_zipper),
            keep_cursor_in_view.before(scroll),
            save_to_file,
        ))
        .add_systems(OnEnter(AppState::Normal), setup_char_zipper)
        .add_systems(OnEnter(AppState::Insert), setup_char_zipper)
        .add_event::<MoveInstruction>()
        .add_event::<GoToChar>()
        .add_event::<MoveChar>()
        .add_event::<DespawnZipper>()
        .add_event::<InsertChar>()
        .add_event::<Save>()
        .init_state::<AppState>()
        .run();
}

// fn ignore_err(_: In<Result<()>>) {}

#[derive(Event)]
pub struct DespawnZipper(Entity);

#[derive(Event, Clone, Copy, Debug)]
pub enum MoveInstruction {
    Parent,
    Left,
    Right,
    Child(usize),
}

#[derive(Event, PartialEq, Eq)]
pub enum MoveChar {
    Right,
    Left,
    LineUp,
    LineDown,
}

#[derive(Component, Reflect)]
pub struct CurrentFocus;

#[derive(Component, Reflect)]
pub struct CurrentZipper;

#[derive(Component, Deref, DerefMut, Reflect)]
pub struct ZipperFocus(Entity);

#[derive(Component, Clone, Reflect)]
pub struct ZipperSiblings { left: Vec<Entity>, right: VecDeque<Entity> }

fn setup(mut commands: Commands) {
    commands.spawn((Camera2dBundle::default(), MainCamera));
    commands.spawn(PerfUiCompleteBundle::default());
}

fn setup_root_zipper(
    mut commands: Commands,
    root_window_q: Query<Entity, (With<AppWindow>, Without<Parent>)>
) {
    let focus = root_window_q.single();
    commands.spawn((
        CurrentZipper,
        RootZipperBundle::new(ZipperType::Window, focus) 
    ));
    commands.entity(focus).insert(CurrentFocus);
}

fn setup_char_zipper(
    mut move_inst_evw: EventWriter<MoveInstruction>,
    mut next_state: ResMut<NextState<AppState>>,
    curr_zipp_q: Query<&ZipperType, With<CurrentZipper>>,
) {
    match curr_zipp_q.single() {
        ZipperType::Document => {
            move_inst_evw.send(MoveInstruction::Child(0));
            move_inst_evw.send(MoveInstruction::Child(0));
            move_inst_evw.send(MoveInstruction::Child(0));
        },
        ZipperType::Line => {
            move_inst_evw.send(MoveInstruction::Child(0));
            move_inst_evw.send(MoveInstruction::Child(0));
        },
        ZipperType::Span => {
            move_inst_evw.send(MoveInstruction::Child(0));
        },
        ZipperType::Character => (),
        _ => next_state.set(AppState::Travel),
    }
}

#[derive(Event)]
pub struct Save;

fn save_to_file(
    mut save_evr: EventReader<Save>,
    file_path: Res<WorkingFilePath>,
    text_q: Query<&Text, With<Character>>,
    doc_q: Query<&Children, With<Document>>,
    content_q: Query<&Children, Or<(With<Line>, With<Span>)>>,
) {
    for _ in save_evr.read() {
        println!("save");
        let mut output = String::new();
        let doc_children = doc_q.single();
        for line_id in doc_children.iter() {
            let line_children = content_q.get(*line_id).unwrap();
            for span_id in line_children.iter() {
                let span_children = content_q.get(*span_id).unwrap();
                for ch_id in span_children.iter() {
                    let character = text_q.get(*ch_id).unwrap();
                    output.push_str(
                        character
                            .sections
                            .iter()
                            .fold(String::new(), |mut acc, s| {
                                let ch = s.downcast_ref::<TextSection>().unwrap().value.as_str();
                                acc.push_str(ch);
                                acc
                            })
                            .as_str()
                    )
                }
            }
            output.push_str("\n");
        }
        fs::write(file_path.clone(), output).unwrap();
    }
}

fn control_normal(
    mut char_input_evr: EventReader<ReceivedCharacter>,
    mut char_movement_evw: EventWriter<MoveChar>,
    mut save_evw: EventWriter<Save>,
    mut next_state: ResMut<NextState<AppState>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    for char in char_input_evr.read() {
        match char.char.as_str() {
            "h" => { char_movement_evw.send(MoveChar::Left); },
            "l" => { char_movement_evw.send(MoveChar::Right); },
            "j" => { char_movement_evw.send(MoveChar::LineDown); },
            "k" => { char_movement_evw.send(MoveChar::LineUp); },
            "i" => next_state.set(AppState::Insert),
            "t" if keys.pressed(KeyCode::ControlLeft) => next_state.set(AppState::Travel),
            "s" if keys.pressed(KeyCode::ControlLeft) => { save_evw.send(Save); },
            _ => ()
        }
    }
}

#[derive(Event)]
pub enum InsertChar{
    Str(String),
    Delete,
    ForwardDelete
}

fn process_insert(
    mut commands: Commands,
    mut insert_evr: EventReader<InsertChar>,
    chars_q: Query<&Parent, With<Character>>,
    mut curr_zip_q: Query<(&ZipperType, &mut ZipperFocus, &mut ZipperSiblings), With<CurrentZipper>>,
    mut move_inst_evw: EventWriter<MoveInstruction>,
) {
    for input in insert_evr.read() {
        let (zipp_type, mut focus, mut siblings) = curr_zip_q.single_mut();
        if *zipp_type != ZipperType::Character { return }
        let curr_index = siblings.left.len();
        let span_id = chars_q.get(**focus).unwrap();
        match input {
            InsertChar::Str(str) => {
                let char_id = commands.spawn((
                    Character,
                    TextBundle::from_section(str, Default::default())
                )).id();
                commands.entity(**span_id).insert_children(curr_index, &[char_id]);
                siblings.left.push(char_id);
            },
            InsertChar::Delete => {
                if siblings.left.len() != 0 {
                    commands.entity(**span_id)
                        .remove_children(&[siblings.left.pop().unwrap()]);
                } else {
                    commands.entity(**span_id)
                        .remove_children(&[**focus]);
                    if siblings.right.len() != 0 {
                        *focus = ZipperFocus(siblings.right.pop_front().unwrap());
                    } else {
                        move_inst_evw.send(MoveInstruction::Parent);
                    }
                }
            },
            InsertChar::ForwardDelete => {
                commands.entity(**span_id)
                    .remove_children(&[**focus]);
                if siblings.right.len() != 0 {
                    *focus = ZipperFocus(siblings.right.pop_front().unwrap());
                } else if siblings.left.len() != 0 {
                    *focus = ZipperFocus(siblings.left.pop().unwrap());
                } else {
                    move_inst_evw.send(MoveInstruction::Parent);
                }
            },
        }
    }
}

fn control_insert(
    mut save_evw: EventWriter<Save>,
    mut char_input_evr: EventReader<ReceivedCharacter>,
    mut keyb_input_evr: EventReader<KeyboardInput>,
    mut insert_evw: EventWriter<InsertChar>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let mut control_pressed = false;
    for key in keyb_input_evr.read() {
        use KeyCode::*;
        use ButtonState::*;
        match (key.key_code, key.state) {
            (Escape, Pressed) => {
                next_state.set(AppState::Normal);
                char_input_evr.clear();
            }
            (Delete, Pressed) => {
                insert_evw.send(InsertChar::ForwardDelete);
                char_input_evr.clear();
            },
            (Backspace, Pressed) => {
                insert_evw.send(InsertChar::Delete);
                char_input_evr.clear();
            },
            (ControlLeft, Pressed) => {
                control_pressed = true;
            },
            _ => (),
        }
    }

    for char in char_input_evr.read() {
        if control_pressed {
            match char.char.as_str() {
                "t" => next_state.set(AppState::Travel),
                "s" => { save_evw.send(Save); },
                _ => (),
            }
            return;
        }
        insert_evw.send(InsertChar::Str(char.char.to_string()));
    }
}

fn control_travel(
    mut char_input_evr: EventReader<ReceivedCharacter>,
    mut save_evw: EventWriter<Save>,
    mut zipper_movement_evw: EventWriter<MoveInstruction>,
    mut next_state: ResMut<NextState<AppState>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Normal);
    }
    for char in char_input_evr.read() {
        match char.char.as_str() {
            "s" if keys.pressed(KeyCode::ControlLeft) => { save_evw.send(Save); },
            "h" | "a" => { zipper_movement_evw.send(MoveInstruction::Left); },
            "l" | "d" => { zipper_movement_evw.send(MoveInstruction::Right); },
            "j" | "w" => { zipper_movement_evw.send(MoveInstruction::Child(0)); },
            "k" | "s" => { zipper_movement_evw.send(MoveInstruction::Parent); },
            "i" => next_state.set(AppState::Insert),
            _ => ()
        }
    }
}

fn move_char_left_right(
    mut move_char_evr: EventReader<MoveChar>,
    mut move_zipp_evw: EventWriter<MoveInstruction>,
    curr_zipp_q: Query<(&Parent, &ZipperType, &ZipperSiblings), With<CurrentZipper>>,
    zippers_q: Query<&ZipperSiblings>,
) {
    for movement in move_char_evr.read() {
        if *movement == MoveChar::LineUp || *movement == MoveChar::LineDown { return }
        let (zip_parent, zip_type, siblings) = curr_zipp_q.single();
        if *zip_type != ZipperType::Character { return }
        let par_sibs = zippers_q.get(**zip_parent).unwrap();
        match movement {
            MoveChar::Left => {
                if siblings.left.len() > 0 {
                    move_zipp_evw.send(MoveInstruction::Left);
                } else if par_sibs.left.len() > 0 {
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Left);
                    move_zipp_evw.send(MoveInstruction::Child(usize::MAX));
                }
            },
            MoveChar::Right => {
                if siblings.right.len() > 0 {
                    move_zipp_evw.send(MoveInstruction::Right);
                } else if par_sibs.right.len() > 0 {
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Right);
                    move_zipp_evw.send(MoveInstruction::Child(0));
                }
            },
            _ => (),
        }
    }
}

fn move_char_up_down (
    mut move_char_evr: EventReader<MoveChar>,
    mut move_zipp_evw: EventWriter<MoveInstruction>,
    mut move_line_evr: EventWriter<GoToChar>,
    main_q: Query<&Children, Or<(With<Line>, With<Span>)>>,
    zippers_q: Query<(&Parent, &ZipperSiblings)>,
    curr_zipp_q: Query<(&Parent, &ZipperType, &ZipperSiblings), With<CurrentZipper>>,
) {
    for movement in move_char_evr.read() {
        if *movement == MoveChar::Left || *movement == MoveChar::Right { return }
        let (parent, zip_type, siblings) = curr_zipp_q.single();
        if *zip_type != ZipperType::Character { return }
        let (span_zip_par, span_zip_sibs) = zippers_q.get(**parent).unwrap();
        let mut curr_pos = span_zip_sibs.left.iter().fold(0_usize, |acc, sib| {
            let id = sib.downcast_ref::<Entity>().unwrap();
            let span = main_q.get(*id).unwrap();
            acc + span.len()
        });
        curr_pos += siblings.left.len();

        let (_, line_zip_sibs) = zippers_q.get(**span_zip_par).unwrap();

        match movement {
            MoveChar::LineUp => {
                if let Some(line_id) = line_zip_sibs.left.last() {
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Left);
                    move_line_evr.send(GoToChar(curr_pos, *line_id));
                }
            },
            MoveChar::LineDown => {
                if let Some(line_id) = line_zip_sibs.right.front() {
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Right);
                    move_line_evr.send(GoToChar(curr_pos, *line_id));
                }
            },
            _ => (),
        }
    }
}

#[derive(Event)]
pub struct GoToChar(usize, Entity);

fn goto_char(
    mut char_evr: EventReader<GoToChar>,
    mut zipper_movement_evw: EventWriter<MoveInstruction>,
    main_q: Query<&Children, Or<(With<Line>, With<Span>)>>,
) {
    'event: for GoToChar(position, line_id) in char_evr.read() {
        let line_children = main_q.get(*line_id).unwrap();
        let mut curr_char_pos = 0_usize;
        let mut span_count = 0_usize;
        for span_id in line_children.iter() {
            let span_children = main_q.get(*span_id).unwrap();
            if curr_char_pos + span_children.len() > *position {
                zipper_movement_evw.send(MoveInstruction::Child(span_count));
                let mut char_count = 0_usize;
                for _ in span_children.iter() {
                    if curr_char_pos + char_count == *position {
                        zipper_movement_evw.send(MoveInstruction::Child(char_count));
                        continue 'event;
                    }
                    char_count += 1;
                }
                zipper_movement_evw.send(MoveInstruction::Child(usize::MAX));
                continue 'event;
        }
            curr_char_pos += span_children.len();
            span_count += 1;
        }
        zipper_movement_evw.send(MoveInstruction::Child(usize::MAX));
        zipper_movement_evw.send(MoveInstruction::Child(usize::MAX));
    }
}

fn move_zipper(
    world: &mut World,
    mut state: Local<SystemState<(
        Commands,
        EventReader<MoveInstruction>,
        Query<
            (
                Entity,
                &mut ZipperFocus,
                &ZipperType,
                Option<&mut ZipperSiblings>,
                Option<&Parent>
            ),
            With<CurrentZipper>
        >,
        Query<&ZipperFocus, Without<CurrentZipper>>,
        Query<
            &Children,
            Or<(
                With<AppWindow>,
                With<Document>,
                With<Line>,
                With<Span>,
            )>
        >,
    )>>
) {
    let mut inst_events = Vec::with_capacity(5);
    let (_, mut events, _, _, _) = state.get_mut(world);
    for i in events.read() { inst_events.push(*i) }

    for inst in inst_events.into_iter() {
        {
            let (
                mut commands,
                _,
                mut curr_zipper_q,
                zippers_q,
                app_tree_q
            ) = state.get_mut(world);
            match inst {
                MoveInstruction::Left => {
                    let (_, mut curr_focus, _, siblings, _) = curr_zipper_q.single_mut();
                    if siblings.is_none() { return }
                    // adjust focus and siblings
                    let mut sibs = siblings.unwrap();
                    if sibs.clone().left.len() == 0 { return }

                    commands.entity(**curr_focus).remove::<CurrentFocus>();

                    sibs.right.push_front(**curr_focus);
                    *curr_focus = ZipperFocus(sibs.left.pop().unwrap());

                    commands.entity(**curr_focus).insert(CurrentFocus);
                },
                MoveInstruction::Right => {
                    let (_, mut curr_focus, _, siblings, _) = curr_zipper_q.single_mut();
                    if siblings.is_none() { return }
                    // adjust focus and siblings
                    let mut sibs = siblings.unwrap();
                    if sibs.clone().right.len() == 0 { return }

                    commands.entity(**curr_focus).remove::<CurrentFocus>();

                    sibs.left.push(**curr_focus);
                    *curr_focus = ZipperFocus(sibs.right.pop_front().unwrap());

                    commands.entity(**curr_focus).insert(CurrentFocus);
                },
                MoveInstruction::Parent => {
                    let (curr_id, curr_focus, _, _, parent) = curr_zipper_q.single_mut();
                    if parent.is_none() { return }
                    let &ZipperFocus(focus) = zippers_q.get(**parent.unwrap()).unwrap();

                    commands.entity(**curr_focus).remove::<CurrentFocus>();
                    commands.entity(focus).insert(CurrentFocus);
                    commands.entity(**parent.unwrap()).insert(CurrentZipper);

                    commands.entity(curr_id).despawn_recursive();
                },
                MoveInstruction::Child(index) => {
                    if curr_zipper_q.is_empty() { return }
                    let (curr_id, curr_focus, curr_type, _, _,) = curr_zipper_q.single();

                    match curr_type {
                        ZipperType::Character => return,
                        _ => (),
                    };

                    let curr_zipper_children = app_tree_q.get(**curr_focus).unwrap();

                    if curr_zipper_children.len() == 0 { return }

                    let index = min(index, curr_zipper_children.len() - 1);
                    let (left, right_tmp) = curr_zipper_children.split_at(index);
                    let (new_focus, right) = right_tmp.split_at(1);
                    let new_focus = new_focus[0];

                    commands.entity(curr_id).remove::<CurrentZipper>();
                    commands.entity(**curr_focus).remove::<CurrentFocus>();
                    commands.entity(new_focus).insert(CurrentFocus);
                    let new_zip_id = commands.spawn((
                        CurrentZipper,
                        BranchZipperBundle::new(
                            curr_type.child_type(),
                            new_focus,
                            left.into(),
                            right.to_vec().into(),
                        )
                    )).id();
                    commands.entity(curr_id).add_child(new_zip_id);
                },
            }
        }
        state.apply(world);
    }
}

fn despawn_zipper(
    mut commands: Commands,
    mut zip_evr: EventReader<DespawnZipper>
) {
    for DespawnZipper(id) in zip_evr.read() {
        commands.entity(*id).despawn_recursive();
    }
}

fn highlight_border(
    mut commands: Commands,
    new_focus: Query<Entity, Added<CurrentFocus>>,
) {
    for curr_focus in new_focus.iter() {
        commands
            .entity(curr_focus)
            .insert(Outline::new(Val::Px(1.), Val::Px(0.), Color::WHITE));
    }
}

fn dehighlight_border(
    mut commands: Commands,
    mut removed: RemovedComponents<CurrentFocus>,
) {
    for id in removed.read() {
        commands
            .entity(id)
            .remove::<Outline>();
    }
}

fn keep_cursor_in_view(
    mut scroll_evw: EventWriter<Scroll>,
    app_tree_q: Query<&GlobalTransform, With<Node>>,
    curr_zipp_q: Query<&ZipperFocus, Added<CurrentZipper>>,
    cam_q: Query<&OrthographicProjection>,
) {
    if curr_zipp_q.is_empty() { return }

    let cam_proj = cam_q.single();
    let g_translation = app_tree_q
        .get(**curr_zipp_q.single()).unwrap()
        .compute_transform()
        .translation;

    let cam_offset = cam_proj.area.max.y;
    if g_translation.y < cam_proj.area.min.y + cam_offset {
        scroll_evw.send(Scroll(12.));
    } else if g_translation.y > cam_proj.area.max.y + cam_offset {
        scroll_evw.send(Scroll(-12.));
    }
}

#[derive(Component, Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZipperType {
    Window,
    Document,
    Line,
    Span,
    Character,
}

impl ZipperType {
    pub fn child_type(&self) -> Self {
        match self {
            ZipperType::Window => ZipperType::Document,
            ZipperType::Document => ZipperType::Line,
            ZipperType::Line => ZipperType::Span,
            ZipperType::Span => ZipperType::Character,
            ZipperType::Character => ZipperType::Character,
        }
    }
}

#[derive(Bundle)]
pub struct RootZipperBundle {
    zipper_type: ZipperType,
    focus: ZipperFocus,
}

impl RootZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
        }
    }
}

#[derive(Bundle)]
pub struct BranchZipperBundle {
    zipper_type: ZipperType,
    focus: ZipperFocus,
    siblings: ZipperSiblings,
}

impl BranchZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        left: Vec<Entity>,
        right: VecDeque<Entity>,
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            siblings: ZipperSiblings { left, right },
        }
    }
}

#[derive(Bundle)]
pub struct LeafZipperBundle {
    zipper_type: ZipperType,
    focus: ZipperFocus,
    siblings: ZipperSiblings,
}

#[derive(Bundle)]
pub struct SpanZipperBundle {
    zipper_type: ZipperType,
    focus: ZipperFocus,
    siblings: ZipperSiblings,
}

impl SpanZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        left: Vec<Entity>,
        right: VecDeque<Entity>,
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            siblings: ZipperSiblings { left, right },
        }
    }
}

impl LeafZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        left: Vec<Entity>,
        right: VecDeque<Entity>
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            siblings: ZipperSiblings { left, right },
        }
    }
}

