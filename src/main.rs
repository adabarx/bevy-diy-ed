use std::{cmp::min, collections::VecDeque};

use bevy::{
    ecs::system::SystemState, prelude::*, reflect::List, winit::WinitSettings
};
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin};

mod text_components;

use text_components::{AppWindow, Document, DocumentPlugin, Line, Scroll, ScrollPosition, Span};

#[derive(Component)]
pub struct MainCamera;

#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AppState {
    #[default]
    Normal,
    Insert,
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
        .add_plugins(PerfUiPlugin)
        .add_plugins(DocumentPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, (setup, setup_root_zipper).chain())
        .add_systems(Update, (
            control,
            (move_char_left_right, move_char_up_down)
                .before(goto_char)
                .after(control),
            highlight_border,
            dehighlight_border,
            despawn_zipper,
            move_zipper,
            goto_char.before(move_zipper),
            keep_cursor_in_view
        ))
        .add_event::<MoveInstruction>()
        .add_event::<GoToChar>()
        .add_event::<MoveChar>()
        .add_event::<DespawnZipper>()
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

fn control(
    mut char_input_evr: EventReader<ReceivedCharacter>,
    mut zipper_movement_evw: EventWriter<MoveInstruction>,
    mut char_movement_evw: EventWriter<MoveChar>,
    curr_zipp_q: Query<&ZipperType, With<CurrentZipper>>,
) {
    use ZipperType::*;
    for char in char_input_evr.read() {
        let zip_type = curr_zipp_q.single();
        match char.char.as_str() {
            "h" if *zip_type == Character => { char_movement_evw.send(MoveChar::Left); },
            "l" if *zip_type == Character => { char_movement_evw.send(MoveChar::Right); },
            "j" if *zip_type == Character => { char_movement_evw.send(MoveChar::LineDown); },
            "k" if *zip_type == Character => { char_movement_evw.send(MoveChar::LineUp); },

            "h" => { zipper_movement_evw.send(MoveInstruction::Left); },
            "l" => { zipper_movement_evw.send(MoveInstruction::Right); },
            "j" => { zipper_movement_evw.send(MoveInstruction::Child(0)); },
            "k" => { zipper_movement_evw.send(MoveInstruction::Parent); },

            "a" => { zipper_movement_evw.send(MoveInstruction::Left); },
            "d" => { zipper_movement_evw.send(MoveInstruction::Right); },
            "w" => { zipper_movement_evw.send(MoveInstruction::Child(0)); },
            "s" => { zipper_movement_evw.send(MoveInstruction::Parent); },
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
    for GoToChar(position, line_id) in char_evr.read() {
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
                        return;
                    }
                    char_count += 1;
                }
                zipper_movement_evw.send(MoveInstruction::Child(usize::MAX));
                return;
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

    println!("curr_zipp x: {} y: {}", g_translation.x, g_translation.y);

    if g_translation.y < cam_proj.area.min.y + 360. {
        scroll_evw.send(Scroll(12.));
    } else if g_translation.y > cam_proj.area.max.y + 360. {
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

