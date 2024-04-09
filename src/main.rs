use std::{cmp::min, collections::VecDeque};

use bevy::{
    ecs::system::SystemState, prelude::*, winit::WinitSettings
};
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin};

mod text_components;

use text_components::{AppWindow, Document, DocumentPlugin, Line, Span};

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
            control.before(move_char),
            move_char.before(move_zipper_world),
            highlight_border,
            dehighlight_border,
            which_zipper,
            despawn_zipper,
            move_zipper_world,
        ))
        .add_event::<MoveInstruction>()
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

#[derive(Event)]
pub enum MoveChar {
    Right,
    Left,
    // LineUp,
    // LineDown,
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

fn move_char(
    mut move_char_evr: EventReader<MoveChar>,
    mut move_zipp_evw: EventWriter<MoveInstruction>,
    curr_zipp_q: Query<(&ZipperType, &ZipperSiblings), With<CurrentZipper>>,
) {
    for movement in move_char_evr.read() {
        let (zip_type, siblings) = curr_zipp_q.single();
        if *zip_type != ZipperType::Character { return }
        match movement {
            MoveChar::Left => {
                if siblings.left.len() > 0 {
                    // move to left sibling if able
                    move_zipp_evw.send(MoveInstruction::Left);
                } else {
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Left);
                    move_zipp_evw.send(MoveInstruction::Child(usize::MAX));
                }
            },
            MoveChar::Right => {
                if siblings.right.len() > 0 {
                    move_zipp_evw.send(MoveInstruction::Right);
                } else {
                    move_zipp_evw.send(MoveInstruction::Parent);
                    move_zipp_evw.send(MoveInstruction::Right);
                    move_zipp_evw.send(MoveInstruction::Child(0));
                }
            },
            // MoveChar::LineUp => (),
            // MoveChar::LineDown => (),
        }
    }
}

fn move_zipper_world(
    world: &mut World,
    mut state: Local<SystemState<(
        Commands,
        EventReader<MoveInstruction>,
        Query<
            (Entity, &mut ZipperFocus, &ZipperType, Option<&mut ZipperSiblings>, Option<&Parent>),
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
    let mut inst_events = Vec::with_capacity(3);
    let (_, mut events, _, _, _) = state.get_mut(world);
    for i in events.read() {
        inst_events.push(*i);
    }

    for inst in inst_events.into_iter() {
        {
            let (mut commands, _, mut curr_zipper_q, zippers_q, app_tree_q) = state.get_mut(world);
            let (
                curr_id,
                mut curr_focus,
                _curr_zipper_type,
                siblings,
                parent
            ) = curr_zipper_q.single_mut();
            match inst {
                MoveInstruction::Left if siblings.is_some() => {
                    // adjust focus and siblings
                    let mut sibs = siblings.unwrap();
                    if sibs.clone().left.len() == 0 { return }

                    commands.entity(**curr_focus).remove::<CurrentFocus>();

                    sibs.right.push_front(**curr_focus);
                    *curr_focus = ZipperFocus(sibs.left.pop().unwrap());

                    commands.entity(**curr_focus).insert(CurrentFocus);
                },
                MoveInstruction::Right if siblings.is_some() => {
                    // adjust focus and siblings
                    let mut sibs = siblings.unwrap();
                    if sibs.clone().right.len() == 0 { return }

                    commands.entity(**curr_focus).remove::<CurrentFocus>();

                    sibs.left.push(**curr_focus);
                    *curr_focus = ZipperFocus(sibs.right.pop_front().unwrap());

                    commands.entity(**curr_focus).insert(CurrentFocus);
                },
                MoveInstruction::Parent if parent.is_some() => {
                    let &ZipperFocus(focus) = zippers_q.get(**parent.unwrap()).unwrap();

                    commands.entity(**curr_focus).remove::<CurrentFocus>();
                    commands.entity(focus).insert(CurrentFocus);
                    commands.entity(**parent.unwrap()).insert(CurrentZipper);

                    commands.entity(curr_id).despawn_recursive();
                },
                MoveInstruction::Child(index) => {
                    if curr_zipper_q.is_empty() { return }
                    let (
                        curr_zipper_id,
                        curr_zipper_focus,
                        curr_zipper_type,
                        _,
                        _,
                    ) = curr_zipper_q.single();

                    match curr_zipper_type {
                        ZipperType::Character => return,
                        _ => (),
                    };

                    let curr_zipper_children = app_tree_q.get(**curr_zipper_focus).unwrap();

                    if curr_zipper_children.len() == 0 { return }

                    let index = min(index, curr_zipper_children.len() - 1);
                    let (left, right_tmp) = curr_zipper_children.split_at(index);
                    let (new_focus, right) = right_tmp.split_at(1);
                    let new_focus = new_focus[0];

                    commands.entity(curr_zipper_id).remove::<CurrentZipper>();
                    commands.entity(**curr_zipper_focus).remove::<CurrentFocus>();
                    commands.entity(new_focus).insert(CurrentFocus);
                    let new_zip_id = commands.spawn((
                        CurrentZipper,
                        BranchZipperBundle::new(
                            curr_zipper_type.child_type(),
                            new_focus,
                            left.into(),
                            right.to_vec().into(),
                        )
                    )).id();
                    commands.entity(curr_zipper_id).add_child(new_zip_id);

                },
                _ => (),
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

fn which_zipper(
    new_focus: Query<&ZipperType, Added<CurrentZipper>>,
) {
    for zip_type in new_focus.iter() {
        match zip_type {
            ZipperType::Window => println!("Window"),
            ZipperType::Document => println!("Document"),
            ZipperType::Line => println!("Line"),
            ZipperType::Span => println!("Span"),
            ZipperType::Character => println!("Character"),
        }
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

#[derive(Component, Reflect, Clone, Copy, PartialEq, Eq)]
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

