// The tablet's non-pausing dialog host, renderer and input routing.
use super::{Game, Screen, astral_cruiser as runtime};
use super::astral_tablet::{self as tablet, Hit, Layout, Rect, Tab, TabletAction, TabletState};
use super::astral_lang::{resolve, tr, Lang, LangSetting};
use super::astral_tablet_map::TabletMap;
pub use super::astral_save::Waypoint;
use glam::DVec3;
use horizon_client::{clipboard::EditShortcut, hud::{HudBatch, rgba}, ui::EditBox};
use std::{cell::RefCell, ops::ControlFlow, sync::atomic::{AtomicBool, Ordering}};
use winit::keyboard::KeyCode;

pub struct WeaponRow {
    pub name: &'static str,
    pub blurb: &'static str,
    pub color: [f32; 4],
    pub cooldown_left: f64,
    pub cooldown: f64,
    pub energy: f64,
    pub reach: f64,
    pub waves: usize,
    pub group: u8,
}

pub struct TabletData {
    pub weapons: Vec<WeaponRow>,
    pub energy: f64,
    pub capacity: f64,
    pub heat: [f64; 4],
    pub speed: f64,
    pub altitude: f64,
    pub pos: DVec3,
    pub dim: String,
    pub heading: f64,
    pub aim: Option<DVec3>,
    pub waypoints: Vec<Waypoint>,
    pub shake: f32,
    pub volume: f32,
    pub orbit: bool,
    pub lang: LangSetting,
}

struct Open {
    state: TabletState,
    edits: [EditBox; 5],
    map: TabletMap,
    dragging: Option<usize>,
    selecting: bool,
    error: String,
}

thread_local! { static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) }; }
static HOLD: AtomicBool = AtomicBool::new(false);

pub(super) fn holding() -> bool { HOLD.load(Ordering::Relaxed) }

/// HEAD of every dialog replacement/clear: never capture an unrelated dialog.
pub(super) fn reset() {
    OPEN.with_borrow_mut(|slot| *slot = None);
    HOLD.store(false, Ordering::Relaxed);
}

pub(super) fn sync_host(game: &Game<'_>) {
    if !game.dialog_open() || game.screen != Screen::Playing || game.player.is_none()
        || !game.native_on("astral_cruiser") { reset(); }
}

pub(super) fn close(game: &mut Game<'_>) {
    reset();
    game.clear_dialog();
}

pub(super) fn open(game: &mut Game<'_>, data: TabletData, selected: usize) {
    // A notice is the smallest valid dialog; the tablet draws over it and takes its input.
    let marker = r#"{type:"minecraft:notice",title:"astral_cruiser:tablet",pause:false,can_close_with_escape:true}"#;
    let Ok((tag, _)) = horizon_world::nbt::parse_snbt(marker) else { return; };
    game.show_dialog(&tag);
    if !game.dialog_open() { return; }
    let aim = data.aim.unwrap_or(data.pos);
    let fields = [aim.x, aim.z, aim.x, data.pos.y, aim.z].map(|v| format!("{v:.0}"));
    let edits = std::array::from_fn(|i| {
        let mut edit = EditBox::new(0.0, 0.0, 72.0, 20.0);
        edit.max_length = 24;
        edit.bordered = false;
        edit.focused = false;
        edit.set_value(&fields[i]);
        edit
    });
    OPEN.with_borrow_mut(|slot| *slot = Some(Open {
        state: TabletState { fields, selected, ..TabletState::default() },
        edits, map: TabletMap::default(), dragging: None, selecting: false, error: String::new(),
    }));
    HOLD.store(true, Ordering::Relaxed);
}

fn active(game: &Game<'_>) -> bool {
    sync_host(game);
    OPEN.with_borrow(|slot| slot.is_some())
}

fn fill(game: &Game<'_>, batch: &mut HudBatch, r: Rect, color: u32) {
    game.native_fill(batch, r.array(), color);
}

fn label(game: &Game<'_>, batch: &mut HudBatch, r: Rect, text: &str, color: u32, scale: f32) {
    if scale <= 0.0 { return; }
    let len = game.gui.atlas.font.fit(text, (r.w / scale - 4.0).max(0.0));
    let text: String = text.chars().take(len).collect();
    let start = batch.vertices.len();
    game.native_text(batch, &text, [r.x + 3.0 * scale, r.y + 3.0 * scale], color, false);
    for v in &mut batch.vertices[start..] {
        v.pos[0] = r.x + (v.pos[0] - r.x) * scale;
        v.pos[1] = r.y + (v.pos[1] - r.y) * scale;
    }
}

fn button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, text: &str, scale: f32, chosen: bool) {
    let hover = r.contains(game.cursor.0, game.cursor.1);
    fill(game, batch, r, if hover || chosen { 0xD02B566B } else { 0xB016293B });
    fill(game, batch, Rect { h: scale.max(0.5), ..r }, 0xC059D9FF);
    label(game, batch, r, text, if hover || chosen { 0xFFFFFF } else { 0xB8D8E0 }, scale);
}

fn bar(game: &Game<'_>, batch: &mut HudBatch, r: Rect, share: f64, color: u32) {
    fill(game, batch, r, 0xFF213444);
    fill(game, batch, Rect { w: r.w * share.clamp(0.0, 1.0) as f32, ..r }, 0xFF000000 | color);
}

fn color(row: &WeaponRow) -> u32 {
    let [r, g, b, _] = row.color;
    ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32
}

fn relative(r: Rect, x: f32, y: f32, w: f32, h: f32, s: f32) -> Rect {
    Rect { x: r.x + x*s, y: r.y + y*s, w: w*s, h: h*s }
}

fn field_rect(l: &Layout, i: usize) -> Rect {
    if i < 2 { l.weapon_fields[i] } else { l.nav_fields[i-2] }
}

fn fields(game: &Game<'_>, batch: &mut HudBatch, open: &mut Open, l: &Layout, indices: &[usize]) {
    for &i in indices {
        let r = field_rect(l, i);
        fill(game, batch, r, 0xFF0B1726);
        fill(game, batch, Rect { h: l.scale, y: r.y+r.h-l.scale, ..r },
            if open.state.focus == Some(i) { 0xFF59D9FF } else { 0xFF446477 });
        let edit = &mut open.edits[i];
        edit.x = r.x + 3.0; edit.y = r.y + 3.0;
        edit.width = (r.w - 6.0).max(1.0); edit.height = (r.h - 6.0).max(1.0);
        edit.focused = open.state.focus == Some(i);
        let start = batch.vertices.len();
        edit.draw(batch, &game.gui.atlas, None);
        batch.clip_since(start, r.x, r.y, r.x + r.w, r.y + r.h);
    }
}

pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> ControlFlow<()> {
    if !active(game) { return ControlFlow::Continue(()); }
    let data = runtime::tablet_data();
    OPEN.with_borrow_mut(|slot| {
        let Some(open) = slot else { return; };
        let l = tablet::layout(batch.width, batch.height);
        let s = l.scale;
        if s <= 0.0 { return; }
        let p = l.panel;
        game.native_fill(batch, [0.0, 0.0, batch.width, batch.height], 0xA8000712);
        batch.fill_gradient(&game.gui.atlas, (p.x, p.y, p.w, p.h), rgba(20,38,56,245), rgba(4,12,24,245));
        // Eight thin quads make a cyan frame with visibly clipped corners.
        let c = 8.0*s;
        let points = [[p.x+c,p.y],[p.x+p.w-c,p.y],[p.x+p.w,p.y+c],[p.x+p.w,p.y+p.h-c],
            [p.x+p.w-c,p.y+p.h],[p.x+c,p.y+p.h],[p.x,p.y+p.h-c],[p.x,p.y+c]];
        for i in 0..8 {
            let a = points[i]; let b = points[(i+1)%8];
            let dx = b[0]-a[0]; let dy = b[1]-a[1]; let len = dx.hypot(dy).max(1.0);
            let n = [-dy/len*s, dx/len*s];
            batch.fill_quad(&game.gui.atlas, [a,b,[b[0]+n[0],b[1]+n[1]],[a[0]+n[0],a[1]+n[1]]], rgba(89,217,255,230));
        }
        let lang = resolve(data.lang, &game.settings.language);
        label(game,batch,relative(p,10.0,7.0,260.0,12.0,s),tr(lang,"tablet.title"),0x73F2FF,s);
        let ticks = (game.world.day_time + 6000).rem_euclid(24000);
        let clock = format!("{:02}:{:02}  {} {:.0}/{:.0}", ticks/1000, ticks%1000*60/1000, tr(lang,"tablet.en_short"), data.energy, data.capacity);
        label(game,batch,relative(p,10.0,23.0,397.0,12.0,s),&clock,0xA6BACB,s);
        button(game,batch,l.close,"×",s,false);
        for (i, &key) in ["tablet.tab.weapons","tablet.tab.map","tablet.tab.navigation","tablet.tab.status"].iter().enumerate() {
            let r = l.tabs[i];
            let chosen = open.state.tab == Tab::ALL[i];
            button(game,batch,r,"",s,chosen);
            // Geometric icons do not depend on optional symbol glyphs in the game font.
            let icon = relative(r,7.0,6.0,10.0,8.0,s);
            fill(game,batch,Rect { w: 2.0*s, ..icon },0xFF59D9FF);
            fill(game,batch,Rect { h: 2.0*s, ..icon },0xFF59D9FF);
            fill(game,batch,Rect { x:icon.x+4.0*s, y:icon.y+(i as f32)*s, w:6.0*s,h:2.0*s },0xFF59D9FF);
            label(game,batch,relative(r,3.0,19.0,82.0,14.0,s),tr(lang,key),0xD0E7F0,s);
            if chosen { fill(game,batch,Rect { w:2.0*s, ..r },0xFF73F2FF); }
        }
        match open.state.tab {
            Tab::Weapons => {
                let start = batch.vertices.len();
                for (i,row) in data.weapons.iter().enumerate() {
                    let r = l.weapon_row(i,open.state.scroll);
                    if r.y+r.h < l.weapon_list.y || r.y > l.weapon_list.y+l.weapon_list.h { continue; }
                    button(game,batch,r,"",s,open.state.selected==i);
                    fill(game,batch,relative(r,3.0,5.0,3.0,12.0,s),0xFF000000|color(row));
                    label(game,batch,relative(r,9.0,2.0,147.0,12.0,s),row.name,color(row),s);
                    let info = format!("{} {:.0}  {:.1}{}  {}{}", tr(lang,"tablet.en_short"), row.energy, row.cooldown_left, tr(lang,"tablet.sec_short"), tr(lang,"tablet.group_short"), row.group+1);
                    label(game,batch,relative(r,9.0,14.0,147.0,12.0,s),&info,0x93AFC0,s);
                    bar(game,batch,relative(r,10.0,27.0,140.0,2.0,s),1.0-row.cooldown_left/row.cooldown.max(0.05),color(row));
                }
                batch.clip_since(start,l.weapon_list.x,l.weapon_list.y,l.weapon_list.x+l.weapon_list.w,l.weapon_list.y+l.weapon_list.h);
                if let Some(row) = data.weapons.get(open.state.selected) {
                    let r = l.weapon_card;
                    label(game,batch,relative(r,0.0,0.0,160.0,12.0,s),row.name,color(row),s);
                    // Wrap the description rather than silently clipping its meaning.
                    let mut line = String::new(); let mut y = 24.0;
                    for word in row.blurb.split_whitespace() {
                        if game.gui.atlas.font.width(&format!("{line} {word}")) > 150.0 && !line.is_empty() {
                            label(game,batch,relative(r,0.0,y,160.0,12.0,s),&line,0xADC8D6,s); y+=12.0; line.clear();
                        }
                        if !line.is_empty() { line.push(' '); } line.push_str(word);
                    }
                    label(game,batch,relative(r,0.0,y,160.0,12.0,s),&line,0xADC8D6,s);
                    for (i,text) in [format!("{}: {:.0} {}", tr(lang,"tablet.radius"), row.reach, tr(lang,"orbit.blocks")),
                        format!("{}: {}", tr(lang,"tablet.waves"), row.waves),
                        format!("{}: {:.0}", tr(lang,"tablet.energy"), row.energy)].iter().enumerate() {
                        label(game,batch,relative(r,0.0,78.0+i as f32*14.0,160.0,12.0,s),text,0xD0E7F0,s);
                    }
                }
                for (i,&key) in ["tablet.target_x","tablet.target_z"].iter().enumerate() {
                    label(game,batch,Rect {y:l.weapon_fields[i].y-12.0*s,..l.weapon_fields[i]},tr(lang,key),0x93AFC0,s);
                }
                fields(game,batch,open,&l,&[0,1]);
                button(game,batch,l.fire,tr(lang,"tablet.fire"),s,false);
            }
            Tab::Map => {
                open.map.draw(game,batch,l.map.array(),data.pos,open.state.zoom);
                map_marks(game,batch,open,&l,&data);
                button(game,batch,l.map_zoom[0],"−",s,false);
                button(game,batch,l.map_zoom[1],"+",s,false);
                button(game,batch,l.target_mode,if open.state.target_dash {tr(lang,"tablet.target_mode_dash")} else {tr(lang,"tablet.target_mode_strike")},s,false);
                let side = l.target_mode;
                let (xi,zi) = if open.state.target_dash {(2,4)} else {(0,1)};
                for (i,text) in [format!("{} {}",open.state.zoom,tr(lang,"tablet.blocks_per_px")),format!("X {}",open.state.fields[xi]),format!("Z {}",open.state.fields[zi])].iter().enumerate() {
                    label(game,batch,relative(side,0.0,34.0+i as f32*16.0,104.0,12.0,s),text,0xC0D9E7,s);
                }
                button(game,batch,l.map_fire,tr(lang,"tablet.fire"),s,false);
                button(game,batch,l.map_dash,tr(lang,"tablet.dash"),s,false);
            }
            Tab::Navigation => {
                for (i,name) in ["X","Y","Z"].iter().enumerate() {
                    label(game,batch,Rect { y:l.nav_fields[i].y-12.0*s,..l.nav_fields[i] },name,0x93AFC0,s);
                }
                fields(game,batch,open,&l,&[2,3,4]);
                button(game,batch,l.nav_dash,tr(lang,"tablet.dash"),s,false);
                button(game,batch,l.waypoint_save,tr(lang,"tablet.save_here"),s,false);
                for (i,point) in data.waypoints.iter().take(8).enumerate() {
                    let r=l.waypoint_rows[i];
                    label(game,batch,Rect {w:216.0*s,..r},&format!("{}  {:.0} {:.0} {:.0}",point.name,point.pos.x,point.pos.y,point.pos.z),0xB8D8E0,s);
                    button(game,batch,l.waypoint_fly[i],tr(lang,"tablet.fly"),s,false);
                    button(game,batch,l.waypoint_delete[i],tr(lang,"tablet.delete"),s,false);
                }
                label(game,batch,relative(l.body,0.0,184.0,326.0,12.0,s),tr(lang,"tablet.open_portal_to"),0x93AFC0,s);
                for (i,&key) in ["tablet.dim.overworld","tablet.dim.nether","tablet.dim.end"].iter().enumerate() { button(game,batch,l.portals[i],tr(lang,key),s,false); }
            }
            Tab::Status => {
                let r=l.body;
                label(game,batch,relative(r,0.0,0.0,326.0,12.0,s),tr(lang,"tablet.energy"),0xB8D8E0,s);
                bar(game,batch,relative(r,0.0,13.0,320.0,6.0,s),data.energy/data.capacity.max(1.0),0x59D9FF);
                for (i,&key) in ["Turrets","Nose guns","Belly beam","Main beam"].iter().enumerate() {
                    label(game,batch,relative(r,0.0,24.0+i as f32*17.0,90.0,12.0,s),tr(lang,key),0xB8D8E0,s);
                    bar(game,batch,relative(r,95.0,28.0+i as f32*17.0,225.0,5.0,s),data.heat[i],0xFF9040);
                }
                let lines=[format!("{} {:.1}  {} {:.0}",tr(lang,"tablet.speed"),data.speed,tr(lang,"tablet.altitude"),data.altitude),
                    format!("X {:.0}  Y {:.0}  Z {:.0}",data.pos.x,data.pos.y,data.pos.z),
                    format!("{} // {}",dimension_name(lang,&data.dim),if data.orbit {tr(lang,"tablet.orbit")} else {tr(lang,"tablet.pilot")})];
                for (i,text) in lines.iter().enumerate() { label(game,batch,relative(r,0.0,97.0+i as f32*16.0,326.0,12.0,s),text,0xB8D8E0,s); }
                button(game,batch,l.status_call,tr(lang,"tablet.btn.call"),s,false);
                button(game,batch,l.status_board,tr(lang,"tablet.btn.board"),s,false);
                button(game,batch,l.status_seat,tr(lang,"tablet.btn.seat"),s,false);
                button(game,batch,l.status_leave,tr(lang,"tablet.btn.leave"),s,false);
                button(game,batch,l.status_reset_onboarding,tr(lang,"tablet.btn.reset_onboarding"),s,false);
                label(game,batch,relative(r,116.0,146.0,52.0,16.0,s),tr(lang,"tablet.lang_label"),0x93AFC0,s);
                button(game,batch,l.status_lang[0],tr(lang,"tablet.lang.auto"),s,data.lang==LangSetting::Auto);
                button(game,batch,l.status_lang[1],tr(lang,"tablet.lang.ru"),s,data.lang==LangSetting::Ru);
                button(game,batch,l.status_lang[2],tr(lang,"tablet.lang.en"),s,data.lang==LangSetting::En);
                for (i,(key,value)) in [("tablet.shake",data.shake),("tablet.volume",data.volume)].iter().enumerate() {
                    let r=l.sliders[i];
                    let name = tr(lang,key);
                    label(game,batch,Rect {y:r.y-13.0*s,..r},&format!("{name}: {:.0}%",value*100.0),0xB8D8E0,s);
                    bar(game,batch,r,f64::from(*value/2.0),0x388BA6);
                    fill(game,batch,Rect {x:r.x+r.w*(*value/2.0).clamp(0.0,1.0)-s,w:2.0*s,..r},0xFF73F2FF);
                }
            }
        }
        if !open.error.is_empty() { label(game,batch,relative(p,104.0,258.0,320.0,11.0,s),&open.error,0xFF8060,s); }
    });
    ControlFlow::Break(())
}

fn dimension_name(lang: Lang, name: &str) -> &str {
    match name {
        "overworld" => tr(lang, "tablet.dim.overworld"),
        "nether" => tr(lang, "tablet.dim.nether"),
        "end" => tr(lang, "tablet.dim.end"),
        _ => name,
    }
}

fn map_marks(game: &Game<'_>, batch: &mut HudBatch, open: &Open, l: &Layout, data: &TabletData) {
    let r=l.map; let [cx,cy]=r.center(); let s=l.scale;
    let start=batch.vertices.len();
    let (xi,zi)=if open.state.target_dash {(2,4)} else {(0,1)};
    if let (Ok(x),Ok(z))=(open.state.fields[xi].parse::<f64>(),open.state.fields[zi].parse::<f64>()) {
        let factor=r.w as f64/(256.0*f64::from(open.state.zoom));
        let tx=cx+((x-data.pos.x)*factor) as f32; let ty=cy+((z-data.pos.z)*factor) as f32;
        if tx.is_finite() && ty.is_finite() {
            if !open.state.target_dash && let Some(row)=data.weapons.get(open.state.selected) {
                let radius=(row.reach*factor) as f32;
                for i in 0..128 { let a=i as f32*std::f32::consts::TAU/128.0;
                    fill(game,batch,Rect {x:tx+a.cos()*radius,y:ty+a.sin()*radius,w:s.max(0.7),h:s.max(0.7)},0xC0000000|color(row)); }
            }
            fill(game,batch,Rect {x:tx-5.0*s,y:ty,w:10.0*s,h:s},0xFFFFD070);
            fill(game,batch,Rect {x:tx,y:ty-5.0*s,w:s,h:10.0*s},0xFFFFD070);
        }
    }
    let (sin,cos)=data.heading.sin_cos();
    let f=[sin as f32,cos as f32]; let right=[f[1],-f[0]];
    let tip=[cx+f[0]*8.0*s,cy+f[1]*8.0*s];
    let a=[cx-f[0]*4.0*s+right[0]*4.0*s,cy-f[1]*4.0*s+right[1]*4.0*s];
    let b=[cx-f[0]*4.0*s-right[0]*4.0*s,cy-f[1]*4.0*s-right[1]*4.0*s];
    batch.fill_quad(&game.gui.atlas,[tip,a,b,b],rgba(115,242,255,255));
    batch.clip_since(start,r.x,r.y,r.x+r.w,r.y+r.h);
}

fn finite(open: &mut Open, i: usize, lang: Lang) -> Option<f64> {
    let value=open.state.fields[i].trim().parse::<f64>().ok().filter(|v| v.is_finite() && v.abs()<=30_000_000.0);
    if value.is_none() { open.error=tr(lang, "tablet.error.coords").into(); }
    value
}

fn choose(game: &Game<'_>, open: &mut Open, hit: Hit, data: &TabletData) -> Option<TabletAction> {
    open.error.clear();
    let lang = resolve(data.lang, &game.settings.language);
    match hit {
        Hit::Tab(tab) => { open.state.tab=tab; open.state.focus=None; }
        Hit::WeaponRow(i) if i<data.weapons.len() => {open.state.selected=i; return Some(TabletAction::Select(i));}
        Hit::Field(i) => {
            open.state.focus=Some(i); open.selecting=true;
            let (w,h)=game.gui_size(); let r=field_rect(&tablet::layout(w,h),i);
            let edit=&mut open.edits[i]; edit.x=r.x+3.0; edit.width=(r.w-6.0).max(1.0);
            edit.click(&game.gui.atlas,game.cursor.0,game.modifiers.shift_key(),false);
        }
        Hit::MapPoint {dx,dz} => {
            let indices=if open.state.target_dash {[2,4]} else {[0,1]};
            for (i,value) in indices.into_iter().zip([data.pos.x+dx,data.pos.z+dz]) {
                open.state.fields[i]=format!("{value:.0}"); open.edits[i].set_value(&open.state.fields[i]);
            }
        }
        Hit::MapZoom(by) => zoom(&mut open.state,by),
        Hit::TargetMode => open.state.target_dash=!open.state.target_dash,
        Hit::Fire if open.state.selected<data.weapons.len() => {
            return Some(TabletAction::Fire {weapon:open.state.selected,x:finite(open,0,lang)?,z:finite(open,1,lang)?});
        }
        Hit::Dash => return Some(TabletAction::Dash {x:finite(open,2,lang)?,y:finite(open,3,lang)?,z:finite(open,4,lang)?}),
        Hit::WaypointSave if data.waypoints.len()<8 => return Some(TabletAction::SaveWaypoint),
        Hit::WaypointFly(i) if i<data.waypoints.len()=>return Some(TabletAction::FlyTo(i)),
        Hit::WaypointDelete(i) if i<data.waypoints.len()=>return Some(TabletAction::DeleteWaypoint(i)),
        Hit::Portal(i)=>return Some(TabletAction::OpenPortal(i)),
        Hit::Slider(i,value)=>{open.dragging=Some(i);return Some(slider_action(i,value));}
        Hit::StatusCall => return Some(TabletAction::CallShip),
        Hit::StatusBoard => return Some(TabletAction::Board),
        Hit::StatusSeat => return Some(TabletAction::Seat),
        Hit::StatusLeave => return Some(TabletAction::Leave),
        Hit::StatusResetOnboarding => return Some(TabletAction::ResetOnboarding),
        Hit::StatusLang(0) => return Some(TabletAction::SetLang(LangSetting::Auto)),
        Hit::StatusLang(1) => return Some(TabletAction::SetLang(LangSetting::Ru)),
        Hit::StatusLang(2) => return Some(TabletAction::SetLang(LangSetting::En)),
        Hit::Close=>return Some(TabletAction::Close),
        _=>{}
    }
    None
}

fn slider_action(i:usize,value:f32)->TabletAction {
    if i==0 {TabletAction::SetShake(value)} else {TabletAction::SetVolume(value)}
}
fn zoom(state:&mut TabletState,by:i32) {
    let current=match state.zoom {4=>2,2=>1,_=>0};
    state.zoom=[1,2,4][(current+by).clamp(0,2) as usize];
}

static CLICK: AtomicBool = AtomicBool::new(false);

/// A click the dialog saw while the tablet is open, for `take_click` next frame.
pub(super) fn queue_click(game: &Game<'_>) -> bool {
    let open = active(game);
    if open {
        CLICK.store(true, Ordering::Relaxed);
    }
    open
}

/// Handles a queued click on the window thread, where the game can be changed.
pub(super) fn take_click(game: &mut Game<'_>) {
    if CLICK.swap(false, Ordering::Relaxed) {
        let _ = click(game);
    }
}

fn click(game: &mut Game<'_>) -> ControlFlow<horizon_client::Result<()>> {
    if !active(game) {return ControlFlow::Continue(());}
    let data=runtime::tablet_data(); let (w,h)=game.gui_size();
    let action=OPEN.with_borrow_mut(|slot| {
        let open=slot.as_mut()?;
        let hit=tablet::hit(&tablet::layout(w,h),&open.state,game.cursor.0,game.cursor.1)?;
        game.audio.play_button_click();
        choose(game,open,hit,&data)
    });
    if let Some(action)=action {runtime::tablet_action(game,action);}
    ControlFlow::Break(Ok(()))
}

pub(super) fn release(game: &mut Game<'_>) -> ControlFlow<()> {
    if !active(game) {return ControlFlow::Continue(());}
    OPEN.with_borrow_mut(|slot| {if let Some(open)=slot {open.dragging=None;open.selecting=false;}});
    ControlFlow::Break(())
}

pub(super) fn drag(game: &mut Game<'_>) -> ControlFlow<()> {
    if !active(game) {return ControlFlow::Continue(());}
    let action=OPEN.with_borrow_mut(|slot| {
        let open=slot.as_mut()?;
        if open.selecting && let Some(i)=open.state.focus { open.edits[i].drag_to(&game.gui.atlas,game.cursor.0); }
        let i=open.dragging?; let (w,h)=game.gui_size(); let r=tablet::layout(w,h).sliders[i];
        Some(slider_action(i,((game.cursor.0-r.x)/r.w.max(1.0)).clamp(0.0,1.0)*2.0))
    });
    if let Some(action)=action {runtime::tablet_action(game,action);}
    ControlFlow::Break(())
}

pub(super) fn scroll(game:&mut Game<'_>,delta:f32)->ControlFlow<()> {
    if !active(game) {return ControlFlow::Continue(());}
    let data=runtime::tablet_data(); let (w,h)=game.gui_size(); let l=tablet::layout(w,h);
    OPEN.with_borrow_mut(|slot| {if let Some(open)=slot {
        if open.state.tab==Tab::Map && l.map.contains(game.cursor.0,game.cursor.1) && delta!=0.0 {
            zoom(&mut open.state,if delta>0.0 {-1} else {1});
        } else if open.state.tab==Tab::Weapons && l.weapon_list.contains(game.cursor.0,game.cursor.1) {
            let max=(data.weapons.len() as f32*30.0-l.weapon_list.h/l.scale.max(0.001)).max(0.0);
            open.state.scroll=(open.state.scroll-delta*30.0).clamp(0.0,max);
        }
    }});
    ControlFlow::Break(())
}

pub(super) fn key(game:&mut Game<'_>,key:KeyCode,shortcut:Option<EditShortcut>,text:Option<&str>)->ControlFlow<horizon_client::Result<()>> {
    if !active(game) {return ControlFlow::Continue(());}
    if matches!(key,KeyCode::Escape|KeyCode::KeyN) {close(game);return ControlFlow::Break(Ok(()));}
    OPEN.with_borrow_mut(|slot| {if let Some(open)=slot {
        if key==KeyCode::Tab {
            let indices:&[usize]=match open.state.tab {Tab::Weapons=>&[0,1],Tab::Navigation=>&[2,3,4],_=>&[]};
            if !indices.is_empty() {
                let current=open.state.focus.and_then(|f|indices.iter().position(|&i|i==f));
                let next=match (current,game.modifiers.shift_key()) {
                    (Some(i),true)=>(i+indices.len()-1)%indices.len(),(Some(i),false)=>(i+1)%indices.len(),
                    (None,true)=>indices.len()-1,(None,false)=>0,
                };
                open.state.focus=Some(indices[next]);
            }
        } else if let Some(i)=open.state.focus {
            let edit=&mut open.edits[i];
            if let Some(shortcut)=shortcut {edit.shortcut(shortcut,&mut game.clipboard);}
            else if !edit.key(key,game.modifiers.control_key(),game.modifiers.shift_key()) {
                let typed:String=text.unwrap_or_default().chars().filter(|c|!c.is_control()).collect();
                edit.insert_text(&typed);
            }
            open.state.fields[i]=edit.value.clone();
        }
    }});
    ControlFlow::Break(Ok(()))
}
