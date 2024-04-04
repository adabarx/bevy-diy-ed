use std::{cmp::min, collections::VecDeque};

use bevy::{
    prelude::*, winit::WinitSettings
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
        .add_systems(Startup, setup)
        .add_systems(PostStartup, setup_root_zipper)
        .add_systems(Update, (
            control.before(move_zipper),
            move_zipper,
            highlight_border,
            dehighlight_border,
            which_zipper,
            despawn_zipper,
            (new_zipper_child, new_zipper_character)
                .after(move_zipper)
                .before(despawn_zipper),
        ))
        .add_event::<NewZipper>()
        .add_event::<ZipperMovement>()
        .add_event::<DespawnZipper>()
        .init_state::<AppState>()
        .run();
}

// fn ignore_err(_: In<Result<()>>) {}

fn setup(mut commands: Commands) {
    commands.spawn((Camera2dBundle::default(), MainCamera));
    commands.spawn(PerfUiCompleteBundle::default());
}

fn setup_root_zipper(
    mut commands: Commands,
    root_window_q: Query<(Entity, &Children), (With<AppWindow>, Without<Parent>)>
) {
    let (focus, children) = root_window_q.single();
    commands.spawn((
        CurrentZipper,
        RootZipperBundle::new(ZipperType::Window, focus, children.to_vec()) 
    ));
    commands.entity(focus).insert(CurrentFocus);
}

fn control(
    mut char_input_evr: EventReader<ReceivedCharacter>,
    mut zipper_movement_evw: EventWriter<ZipperMovement>,
) {
    for char in char_input_evr.read() {
        match char.char.as_str() {
            "h" => _ = zipper_movement_evw.send(ZipperMovement::Left),
            "j" => _ = zipper_movement_evw.send(ZipperMovement::Child(0)),
            "k" => _ = zipper_movement_evw.send(ZipperMovement::Parent),
            "l" => _ = zipper_movement_evw.send(ZipperMovement::Right),
            _ => ()
        }
    }
}

#[derive(Event)]
pub enum ZipperMovement {
    Parent,
    Left,
    Right,
    Child(usize),
}

fn move_zipper(
    mut commands: Commands,
    mut new_zip_evw: EventWriter<NewZipper>,
    mut movement_evr: EventReader<ZipperMovement>,
    mut curr_zipper_q: Query<
        (Entity, &mut ZipperFocus, Option<&mut ZipperSiblings>, Option<&Parent>),
        With<CurrentZipper>
    >,
    zippers_q: Query<&ZipperFocus, Without<CurrentZipper>>,
) {
    for movement in movement_evr.read() {
        let (id, mut curr_focus, siblings, parent) = curr_zipper_q.single_mut();
        match movement {
            ZipperMovement::Left if siblings.is_some() => {
                let mut sibs = siblings.unwrap();
                if sibs.clone().left.len() == 0 { return }

                commands.entity(**curr_focus).remove::<CurrentFocus>();

                sibs.right.push_front(**curr_focus);
                *curr_focus = ZipperFocus(sibs.left.pop().unwrap());

                commands.entity(**curr_focus).insert(CurrentFocus);
            },
            ZipperMovement::Right if siblings.is_some() => {
                let mut sibs = siblings.unwrap();
                if sibs.clone().right.len() == 0 { return }

                commands.entity(**curr_focus).remove::<CurrentFocus>();

                sibs.left.push(**curr_focus);
                *curr_focus = ZipperFocus(sibs.right.pop_front().unwrap());

                commands.entity(**curr_focus).insert(CurrentFocus);
            },
            ZipperMovement::Parent if parent.is_some() => {
                let &ZipperFocus(focus) = zippers_q.get(**parent.unwrap()).unwrap();

                commands.entity(**curr_focus).remove::<CurrentFocus>();
                commands.entity(focus).insert(CurrentFocus);
                commands.entity(**parent.unwrap()).insert(CurrentZipper);

                commands.entity(id).despawn_recursive();
            },
            &ZipperMovement::Child(index) => {
                _ = new_zip_evw.send(NewZipper { index });
            },
            _ => (),
        }
    }
}

#[derive(Event)]
pub struct DespawnZipper(Entity);

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

#[derive(Component, Reflect)]
pub struct CurrentZipper;

#[derive(Component, Reflect)]
pub struct CurrentFocus;

#[derive(Component, Deref, DerefMut, Reflect)]
pub struct ZipperFocus(Entity);

#[derive(Component, Clone, Reflect)]
pub struct ZipperSiblings { left: Vec<Entity>, right: VecDeque<Entity> }

#[derive(Component, Deref, DerefMut, Reflect)]
pub struct ZipperChildren(Vec<Entity>);

#[derive(Component, Clone, Reflect)]
pub struct ZipperCharacter(usize);

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
    children: ZipperChildren,
}

impl RootZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        children: Vec<Entity>
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            children: ZipperChildren(children)
        }
    }
}

#[derive(Bundle)]
pub struct BranchZipperBundle {
    zipper_type: ZipperType,
    focus: ZipperFocus,
    siblings: ZipperSiblings,
    children: ZipperChildren,
}

impl BranchZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        left: Vec<Entity>,
        right: VecDeque<Entity>,
        children: Vec<Entity>
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            siblings: ZipperSiblings { left, right },
            children: ZipperChildren(children)
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

#[derive(Event)]
pub struct NewZipper {
    index: usize,
}

fn new_zipper_child(
    mut new_zipper_evr: EventReader<NewZipper>,
    mut commands: Commands,

    curr_zippers_q: Query<
        (Entity, &ZipperFocus, &ZipperChildren, &ZipperType),
        With<CurrentZipper>
    >,
    full_query: Query<
        &Children,
        Or<(With<AppWindow>, With<Document>, With<Line>, With<Span>)>
    >,
) {
    for &NewZipper { index } in new_zipper_evr.read() {
        let (
            curr_zipper_id,
            curr_zipper_focus,
            curr_zipper_children,
            curr_zip_type
        ) = curr_zippers_q.single();

        match curr_zip_type {
            ZipperType::Span | ZipperType::Character => return,
            _ => (),
        };

        let index = min(index, curr_zipper_children.len());
        let (left, right_tmp) = curr_zipper_children.split_at(index);
        let (new_focus, right) = right_tmp.split_at(1);
        let new_focus = new_focus[0];

        let new_children = full_query.get(new_focus).unwrap(); 

        commands.entity(curr_zipper_id).remove::<CurrentZipper>();
        commands.entity(**curr_zipper_focus).remove::<CurrentFocus>();
        commands.entity(new_focus).insert(CurrentFocus);
        let new_zip_id = commands.spawn((
            CurrentZipper,
            BranchZipperBundle::new(
                curr_zip_type.child_type(),
                new_focus,
                left.into(),
                right.to_vec().into(),
                new_children.to_vec()
            )
        )).id();
        commands.entity(curr_zipper_id).add_child(new_zip_id);
    }
}

fn new_zipper_character(
    mut new_zipper_evr: EventReader<NewZipper>,
    mut commands: Commands,

    curr_zippers_q: Query<
        (Entity, &ZipperFocus, &ZipperType),
        With<CurrentZipper>
    >,
    spans_q: Query<
        &Children,
        (With<Span>, With<CurrentFocus>)
    >,
) {
    for &NewZipper { index } in new_zipper_evr.read() {
        let (curr_zipper_id, curr_zipper_focus, curr_zip_type) = curr_zippers_q.single();
        if *curr_zip_type != ZipperType::Span { return }
        let curr_span_children = spans_q.single();

        let index = min(index, curr_span_children.len());
        let (left, right_tmp) = curr_span_children.split_at(index);
        let (new_focus, right) = right_tmp.split_at(1);
        let new_focus = new_focus[0];

        commands.entity(curr_zipper_id).remove::<CurrentZipper>();
        commands.entity(**curr_zipper_focus).remove::<CurrentFocus>();
        commands.entity(new_focus).insert(CurrentFocus);
        let new_zip_id = commands.spawn((
            CurrentZipper,
            LeafZipperBundle::new(
                ZipperType::Character,
                new_focus,
                left.into(),
                right.to_vec().into(),
            )
        )).id();
        commands.entity(curr_zipper_id).add_child(new_zip_id);
    }
}

