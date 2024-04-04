use std::cmp::min;

use bevy::{
    prelude::*, reflect::List, winit::WinitSettings
};
use iyes_perf_ui::{PerfUiCompleteBundle, PerfUiPlugin};
use anyhow::{Ok, Result};

mod text_components;

use text_components::{AppWindow, Document, DocumentPlugin, Line, Span};

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource)]
pub enum AppState {
    Insert,
    Normal,
    Travel,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin)
        .add_plugins(PerfUiPlugin)
        .add_plugins(DocumentPlugin)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .add_systems(PostStartup, setup_root_zipper)
        .add_systems(Update, (
            control,
            move_zipper,
            highlight_border,
            dehighlight_border,
            which_zipper,
            new_zipper_window.pipe(ignore_err),
            new_zipper_document.pipe(ignore_err),
            new_zipper_line.pipe(ignore_err),
            new_zipper_span.pipe(ignore_err),
            new_zipper_character.pipe(ignore_err),
        ))
        .add_event::<NewZipper>()
        .add_event::<ZipperMovement>()
        .add_event::<ZipperMovement>()
        .run();
}

fn ignore_err(In(result): In<Result<()>>) {
    _ = result;
}

// fn pipe_none(In(result): In<Option<()>>) {
//     let _ = result;
// }

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
    curr_zipper_q: Query<(Entity, &ZipperFocus, Option<&ZipperParent>, Option<&ZipperSiblings>), With<CurrentZipper>>,
    zippers_q: Query<&ZipperFocus, Without<CurrentZipper>>
) {
    for movement in movement_evr.read() {
        let (id, curr_focus, parent, siblings) = curr_zipper_q.single();
        let has_parents = parent.is_some();
        let has_siblings = siblings.is_some();
        match movement {
            ZipperMovement::Left if has_parents && has_siblings =>
                _ = new_zip_evw.send(NewZipper {
                    parent: **parent.unwrap(),
                    index: siblings.unwrap().left.len() - 1
                }),
            ZipperMovement::Right if has_parents && has_siblings =>
                _ = new_zip_evw.send(NewZipper {
                    parent: **parent.unwrap(),
                    index: siblings.unwrap().left.len() + 1
                }),
            &ZipperMovement::Child(index) => 
                _ = new_zip_evw.send(NewZipper {
                    parent: id,
                    index
                }),
            ZipperMovement::Parent if has_parents => {
                let &ZipperFocus(focus) = zippers_q.get(**parent.unwrap()).unwrap();
                commands.entity(**curr_focus).remove::<CurrentFocus>();
                commands.entity(focus).insert(CurrentFocus);
                commands.entity(**parent.unwrap()).insert(CurrentZipper);
                commands.entity(id).despawn_recursive();
            },
            _ => (),
        }
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

// fn highlight_border(
//     mut new_focus: Query<&mut BackgroundColor, Added<CurrentFocus>>
// ) {
//     for mut bgcolor in new_focus.iter_mut() {
//         *bgcolor = BackgroundColor(Color::WHITE);
//     }
// }
//
// fn dehighlight_border(
//     mut removed: RemovedComponents<CurrentFocus>,
//     mut border_q: Query<&mut BackgroundColor>
// ) {
//     for id in removed.read() {
//         println!("removed");
//         if let Ok(mut bgcolor) = border_q.get_mut(id) {
//             *bgcolor = BackgroundColor(Color::BLACK);
//         }
//     }
// }

#[derive(Component, Reflect)]
pub struct CurrentZipper;

#[derive(Component, Reflect)]
pub struct CurrentFocus;

#[derive(Component, Deref, DerefMut, Reflect)]
pub struct ZipperFocus(Entity);

#[derive(Component, Deref, DerefMut, Reflect)]
pub struct ZipperParent(Entity);

#[derive(Component, Reflect)]
pub struct ZipperSiblings { left: Vec<Entity>, right: Vec<Entity> }

#[derive(Component, Deref, DerefMut, Reflect)]
pub struct ZipperChildren(Vec<Entity>);

#[derive(Component, Deref, DerefMut, Reflect)]
pub struct ZipperCharactersChildren(usize);

#[derive(Component, Reflect)]
pub struct ZipperCharacters {
    left: usize,
    right: usize,
    focus: usize,
}

#[derive(Component, Reflect, Clone, Copy)]
pub enum ZipperType {
    Window,
    Document,
    Line,
    Span,
    Character,
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
    parent: ZipperParent,
    siblings: ZipperSiblings,
    children: ZipperChildren,
}

impl BranchZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        parent: Entity,
        left: Vec<Entity>,
        right: Vec<Entity>,
        children: Vec<Entity>
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            parent: ZipperParent(parent),
            siblings: ZipperSiblings { left, right },
            children: ZipperChildren(children)
        }
    }
}

#[derive(Bundle)]
pub struct LeafZipperBundle {
    zipper_type: ZipperType,
    focus: ZipperFocus,
    parent: ZipperParent,
    siblings: ZipperSiblings,
}

#[derive(Bundle)]
pub struct SpanZipperBundle {
    zipper_type: ZipperType,
    focus: ZipperFocus,
    parent: ZipperParent,
    siblings: ZipperSiblings,
    children: ZipperCharactersChildren,
}

impl SpanZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        parent: Entity,
        left: Vec<Entity>,
        right: Vec<Entity>,
        children: usize,
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            parent: ZipperParent(parent),
            siblings: ZipperSiblings { left, right },
            children: ZipperCharactersChildren(children)
        }
    }
}

impl LeafZipperBundle {
    pub fn new(
        zipper_type: ZipperType,
        focus: Entity,
        parent: Entity,
        left: Vec<Entity>,
        right: Vec<Entity>
    ) -> Self {
        Self {
            zipper_type,
            focus: ZipperFocus(focus),
            parent: ZipperParent(parent),
            siblings: ZipperSiblings { left, right },
        }
    }
}

#[derive(Event)]
pub struct NewZipper {
    parent: Entity,
    index: usize,
}

fn new_zipper_window(
    mut new_zipper_evr: EventReader<NewZipper>,
    mut commands: Commands,

    curr_zippers_q: Query<(Entity, &ZipperFocus), With<CurrentZipper>>,
    windows_q: Query<&Children, With<AppWindow>>,
) -> Result<()> {
    for &NewZipper { parent, index } in new_zipper_evr.read() {
        let (curr_id, curr_focus) = curr_zippers_q.single();
        let windows = windows_q.get(**curr_focus)?;

        let index = min(index, windows.len() - 1);
        let (left, right_tmp) = windows.split_at(index);
        let (focus, right) = right_tmp.split_at(1);
        let focus = focus[0];

        let children = windows_q.get(focus)?;

        commands.entity(curr_id).remove::<CurrentZipper>();
        commands.entity(**curr_focus).remove::<CurrentFocus>();
        commands.entity(focus).insert(CurrentFocus);
        commands.spawn((
            CurrentZipper,
            BranchZipperBundle::new(
                ZipperType::Window,
                focus,
                parent,
                left.to_vec(),
                right.to_vec(),
                children.to_vec()
            )
        ));
    }
    Ok(())
}

fn new_zipper_document(
    mut new_zipper_evr: EventReader<NewZipper>,
    mut commands: Commands,

    curr_zippers_q: Query<(Entity, &ZipperFocus), With<CurrentZipper>>,
    windows_q: Query<&Children, With<AppWindow>>,
    documents_q: Query<&Children, With<Document>>,
) -> Result<()> {
    for &NewZipper { parent, index } in new_zipper_evr.read() {
        let (curr_id, curr_focus) = curr_zippers_q.single();
        let windows = windows_q.get(**curr_focus)?;

        let index = min(index, windows.len() - 1);
        let (left, right_tmp) = windows.split_at(index);
        let (focus, right) = right_tmp.split_at(1);
        let focus = focus[0];

        let children = documents_q.get(focus)?;

        commands.entity(curr_id).remove::<CurrentZipper>();
        commands.entity(**curr_focus).remove::<CurrentFocus>();
        commands.entity(focus).insert(CurrentFocus);
        commands.spawn((
            CurrentZipper,
            BranchZipperBundle::new(
                ZipperType::Document,
                focus,
                parent,
                left.to_vec(),
                right.to_vec(),
                children.to_vec()
            )
        ));
    }
    Ok(())
}

fn new_zipper_line(
    mut new_zipper_evr: EventReader<NewZipper>,
    mut commands: Commands,

    curr_zippers_q: Query<(Entity, &ZipperFocus), With<CurrentZipper>>,
    documents_q: Query<&Children, With<Document>>,
    lines_q: Query<&Children, With<Line>>,
) -> Result<()> {
    for &NewZipper { parent, index } in new_zipper_evr.read() {
        let (curr_id, curr_focus) = curr_zippers_q.single();
        let windows = documents_q.get(**curr_focus)?;

        let index = min(index, windows.len() - 1);
        let (left, right_tmp) = windows.split_at(index);
        let (focus, right) = right_tmp.split_at(1);
        let focus = focus[0];

        let children = lines_q.get(focus)?;

        commands.entity(curr_id).remove::<CurrentZipper>();
        commands.entity(**curr_focus).remove::<CurrentFocus>();
        commands.entity(focus).insert(CurrentFocus);
        commands.spawn((
            CurrentZipper,
            BranchZipperBundle::new(
                ZipperType::Line,
                focus,
                parent,
                left.to_vec(),
                right.to_vec(),
                children.to_vec()
            )
        ));
    }
    Ok(())
}

fn new_zipper_span(
    mut new_zipper_evr: EventReader<NewZipper>,
    mut commands: Commands,

    curr_zippers_q: Query<(Entity, &ZipperFocus), With<CurrentZipper>>,
    lines_q: Query<&Children, With<Line>>,
    spans_q: Query<&Text, With<Span>>,
) -> Result<()> {
    for &NewZipper { parent, index } in new_zipper_evr.read() {
        let (curr_id, curr_focus) = curr_zippers_q.single();
        let windows = lines_q.get(**curr_focus)?;

        let index = min(index, windows.len() - 1);
        let (left, right_tmp) = windows.split_at(index);
        let (focus, right) = right_tmp.split_at(1);
        let focus = focus[0];

        let text = spans_q.get(focus)?;

        let len = text.sections.iter().fold(0, |acc, sect| {
            let len = sect.downcast_ref::<TextSection>().unwrap()
                .value
                .chars()
                .count();
            acc + len
        });
        commands.entity(curr_id).remove::<CurrentZipper>();
        commands.entity(**curr_focus).remove::<CurrentFocus>();
        commands.entity(focus).insert(CurrentFocus);
        commands.spawn((
            CurrentZipper,
            SpanZipperBundle::new(
                ZipperType::Span,
                focus,
                parent,
                left.to_vec(),
                right.to_vec(),
                len,
            )
        ));
    }
    Ok(())
}

fn new_zipper_character(
    mut new_zipper_evr: EventReader<NewZipper>,
    mut commands: Commands,

    curr_zippers_q: Query<(Entity, &ZipperFocus), With<CurrentZipper>>,
    spans_q: Query<&Text, With<Span>>,
) -> Result<()> {
    for &NewZipper { parent, index } in new_zipper_evr.read() {
        let (curr_id, curr_focus) = curr_zippers_q.single();
        let text = spans_q.get(parent)?;
        let len = text.sections.iter().fold(0, |acc, sect| {
            let len = sect.downcast_ref::<TextSection>().unwrap()
                .value
                .chars()
                .count();
            acc + len
        });

        let index = min(index, len - 1);
        let left = index.saturating_sub(1);
        let right = (len - index).saturating_sub(1);
        commands.entity(curr_id).remove::<CurrentZipper>();
        commands.entity(**curr_focus).remove::<CurrentFocus>();
        commands.entity(parent).insert((
            CurrentZipper,
            ZipperType::Character,
            ZipperCharacters { left, right, focus: index }
        ));
    }
    Ok(())
}

