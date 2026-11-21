//! Ordem dos sons e colecoes: funcoes puras sobre a config (sem interface), com testes.
//!
//! - `cfg.slots`: lista de sons; a ordem da lista e a ordem na tela.
//! - `slot.collection`: nome da colecao do som, ou None (sem colecao).
//! - `cfg.collections`: colecoes na ordem em que aparecem.
//!
//! Na tela vem primeiro os sons sem colecao e depois cada colecao com os seus sons.
//! Os sons sao identificados pelo `id` (dois slots vazios sao iguais em conteudo).

use crate::config::{Collection, Config, Slot};

pub fn names(cfg: &Config) -> Vec<String> {
    cfg.collections.iter().map(|c| c.name.clone()).collect()
}

pub fn find<'a>(cfg: &'a Config, name: &str) -> Option<&'a Collection> {
    cfg.collections.iter().find(|c| c.name == name)
}

fn clean(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Completa configs antigas ou editadas a mao: nomes em branco, colecoes repetidas e sons
/// apontando para uma colecao que nao existe (ela e criada).
pub fn normalize(cfg: &mut Config) {
    let mut cols: Vec<Collection> = Vec::new();
    for c in &cfg.collections {
        let name = c.name.trim().to_string();
        if !name.is_empty() && !cols.iter().any(|x| x.name == name) {
            cols.push(Collection { name, collapsed: c.collapsed });
        }
    }
    for s in &mut cfg.slots {
        s.collection = s.collection.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(String::from);
        if let Some(name) = &s.collection {
            if !cols.iter().any(|x| &x.name == name) {
                cols.push(Collection { name: name.clone(), collapsed: false });
            }
        }
    }
    cfg.collections = cols;
}

/// Um grupo da tela: a colecao (None = sem colecao) e os seus sons, em ordem.
pub struct Group<'a> {
    pub collection: Option<&'a Collection>,
    pub slots: Vec<&'a Slot>,
}

/// Grupos na ordem da tela: sons sem colecao e depois cada colecao.
pub fn groups(cfg: &Config) -> Vec<Group<'_>> {
    let mut out = vec![Group { collection: None, slots: cfg.slots.iter().filter(|s| s.collection.is_none()).collect() }];
    for c in &cfg.collections {
        let slots = cfg.slots.iter().filter(|s| s.collection.as_deref() == Some(c.name.as_str())).collect();
        out.push(Group { collection: Some(c), slots });
    }
    out
}

/// Som vizinho na mesma colecao (delta -1 = acima, +1 = abaixo), ou None na ponta.
pub fn neighbor(cfg: &Config, id: u64, delta: isize) -> Option<u64> {
    let slot = cfg.slots.iter().find(|s| s.id == id)?;
    let same: Vec<u64> = cfg.slots.iter().filter(|s| s.collection == slot.collection).map(|s| s.id).collect();
    let i = same.iter().position(|&x| x == id)? as isize + delta;
    (0..same.len() as isize).contains(&i).then(|| same[i as usize])
}

/// Troca o som de lugar com o vizinho da mesma colecao. False se ja esta na ponta.
pub fn move_slot(cfg: &mut Config, id: u64, delta: isize) -> bool {
    let Some(other) = neighbor(cfg, id, delta) else { return false };
    let i = cfg.slots.iter().position(|s| s.id == id).unwrap();
    let j = cfg.slots.iter().position(|s| s.id == other).unwrap();
    cfg.slots.swap(i, j);
    true
}

/// Poe o som no fim da colecao `name` (None = sem colecao). Ok(false) se ja estava nela;
/// Err se a colecao nao existe.
pub fn set_collection(cfg: &mut Config, id: u64, name: Option<&str>) -> Result<bool, String> {
    let Some(i) = cfg.slots.iter().position(|s| s.id == id) else { return Ok(false) };
    if cfg.slots[i].collection.as_deref() == name {
        return Ok(false);
    }
    if let Some(n) = name {
        if find(cfg, n).is_none() {
            return Err(n.to_string());
        }
    }
    let mut slot = cfg.slots.remove(i);
    slot.collection = name.map(String::from);
    cfg.slots.push(slot);
    Ok(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Exists,
}

/// Valida o nome de uma colecao (espacos extras saem). Erro junto com o nome limpo.
///
/// `current`: nome atual (ao renomear, pode mudar so maiusculas/minusculas).
/// `reserved`: textos que nao podem virar nome (ex.: o rotulo "(sem colecao)" do menu).
pub fn clean_name(cfg: &Config, name: &str, current: Option<&str>, reserved: &[&str]) -> Result<String, (NameError, String)> {
    let name = clean(name);
    if name.is_empty() {
        return Err((NameError::Empty, name));
    }
    let key = name.to_lowercase();
    let taken = cfg
        .collections
        .iter()
        .filter(|c| Some(c.name.as_str()) != current)
        .map(|c| c.name.to_lowercase())
        .chain(reserved.iter().map(|r| r.to_lowercase()))
        .any(|n| n == key);
    if taken { Err((NameError::Exists, name)) } else { Ok(name) }
}

pub fn add_collection(cfg: &mut Config, name: &str) {
    cfg.collections.push(Collection { name: name.to_string(), collapsed: false });
}

pub fn rename_collection(cfg: &mut Config, old: &str, new: &str) {
    if let Some(c) = cfg.collections.iter_mut().find(|c| c.name == old) {
        c.name = new.to_string();
    }
    for s in cfg.slots.iter_mut().filter(|s| s.collection.as_deref() == Some(old)) {
        s.collection = Some(new.to_string());
    }
}

/// Apaga a colecao; os sons dela vao para o fim dos sons sem colecao (nenhum som e apagado).
pub fn delete_collection(cfg: &mut Config, name: &str) -> usize {
    cfg.collections.retain(|c| c.name != name);
    let (mut moved, kept): (Vec<Slot>, Vec<Slot>) =
        cfg.slots.drain(..).partition(|s| s.collection.as_deref() == Some(name));
    let n = moved.len();
    moved.iter_mut().for_each(|s| s.collection = None);
    cfg.slots = kept;
    cfg.slots.extend(moved);
    n
}

pub fn move_collection(cfg: &mut Config, name: &str, delta: isize) -> bool {
    let Some(i) = cfg.collections.iter().position(|c| c.name == name) else { return false };
    let j = i as isize + delta;
    if !(0..cfg.collections.len() as isize).contains(&j) {
        return false;
    }
    cfg.collections.swap(i, j as usize);
    true
}

pub fn set_collapsed(cfg: &mut Config, name: &str, collapsed: bool) {
    if let Some(c) = cfg.collections.iter_mut().find(|c| c.name == name) {
        c.collapsed = collapsed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: u64, path: &str, collection: Option<&str>) -> Slot {
        Slot { id, path: path.into(), collection: collection.map(String::from), ..Slot::default() }
    }

    fn view(cfg: &Config) -> Vec<(Option<String>, Vec<String>)> {
        groups(cfg)
            .iter()
            .map(|g| (g.collection.map(|c| c.name.clone()), g.slots.iter().map(|s| s.path.clone()).collect()))
            .collect()
    }

    fn v(name: Option<&str>, paths: &[&str]) -> (Option<String>, Vec<String>) {
        (name.map(String::from), paths.iter().map(|p| p.to_string()).collect())
    }

    fn setup() -> Config {
        let mut cfg = Config {
            collections: vec![
                Collection { name: "Memes".into(), collapsed: false },
                Collection { name: "Lula".into(), collapsed: true },
            ],
            slots: vec![
                slot(1, "a", None),
                slot(2, "m1", Some("Memes")),
                slot(3, "b", None),
                slot(4, "l1", Some("Lula")),
                slot(5, "m2", Some("Memes")),
            ],
            ..Config::default()
        };
        normalize(&mut cfg);
        cfg
    }

    #[test]
    fn groups_put_ungrouped_first_then_collections_in_order() {
        let cfg = setup();
        assert_eq!(view(&cfg), [v(None, &["a", "b"]), v(Some("Memes"), &["m1", "m2"]), v(Some("Lula"), &["l1"])]);
    }

    #[test]
    fn move_swaps_only_within_the_same_collection() {
        let mut cfg = setup();
        assert!(move_slot(&mut cfg, 3, -1));
        assert!(move_slot(&mut cfg, 5, -1));
        assert_eq!(view(&cfg), [v(None, &["b", "a"]), v(Some("Memes"), &["m2", "m1"]), v(Some("Lula"), &["l1"])]);
    }

    #[test]
    fn move_at_the_edge_does_nothing() {
        let mut cfg = setup();
        assert!(!move_slot(&mut cfg, 1, -1));
        assert!(!move_slot(&mut cfg, 4, 1)); // unico da colecao Lula
        assert_eq!(cfg.slots.iter().map(|s| s.id).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn move_finds_identical_empty_slots_by_id() {
        let mut cfg = Config { slots: vec![slot(1, "", None), slot(2, "", None), slot(3, "x", None)], ..Config::default() };
        assert!(move_slot(&mut cfg, 2, -1));
        assert_eq!(cfg.slots[0].id, 2);
        assert_eq!(neighbor(&cfg, 2, -1), None);
    }

    #[test]
    fn set_collection_puts_sound_at_the_end_of_the_target() {
        let mut cfg = setup();
        assert_eq!(set_collection(&mut cfg, 1, Some("Memes")), Ok(true));
        assert_eq!(view(&cfg)[1], v(Some("Memes"), &["m1", "m2", "a"]));
        assert_eq!(set_collection(&mut cfg, 1, Some("Memes")), Ok(false));
        assert_eq!(set_collection(&mut cfg, 1, None), Ok(true));
        assert_eq!(view(&cfg)[0], v(None, &["b", "a"]));
        assert!(set_collection(&mut cfg, 1, Some("nao existe")).is_err());
    }

    #[test]
    fn rename_updates_the_sounds() {
        let mut cfg = setup();
        rename_collection(&mut cfg, "Memes", "Engracados");
        assert_eq!(view(&cfg)[1], v(Some("Engracados"), &["m1", "m2"]));
    }

    #[test]
    fn delete_keeps_the_sounds_without_collection() {
        let mut cfg = setup();
        assert_eq!(delete_collection(&mut cfg, "Memes"), 2);
        assert_eq!(view(&cfg), [v(None, &["a", "b", "m1", "m2"]), v(Some("Lula"), &["l1"])]);
        assert_eq!(cfg.slots.len(), 5);
    }

    #[test]
    fn move_collection_and_collapse() {
        let mut cfg = setup();
        assert!(move_collection(&mut cfg, "Lula", -1));
        assert_eq!(names(&cfg), ["Lula", "Memes"]);
        assert!(!move_collection(&mut cfg, "Lula", -1));
        set_collapsed(&mut cfg, "Lula", false);
        assert!(!find(&cfg, "Lula").unwrap().collapsed);
    }

    #[test]
    fn clean_names() {
        let cfg = setup();
        assert_eq!(clean_name(&cfg, "  Novos   sons ", None, &[]), Ok("Novos sons".into()));
        assert_eq!(clean_name(&cfg, "   ", None, &[]).unwrap_err().0, NameError::Empty);
        assert_eq!(clean_name(&cfg, "memes", None, &[]).unwrap_err().0, NameError::Exists);
        assert_eq!(clean_name(&cfg, "MEMES", Some("Memes"), &[]), Ok("MEMES".into()));
        assert_eq!(clean_name(&cfg, "(sem coleção)", None, &["(sem coleção)"]).unwrap_err().0, NameError::Exists);
    }

    #[test]
    fn normalize_old_config() {
        let mut cfg: Config =
            serde_json::from_str(r#"{"slots": [{"key": "f1", "path": "x", "volume": 1.0}, {"path": "y", "collection": " Nova "}]}"#)
                .unwrap();
        normalize(&mut cfg);
        assert_eq!(cfg.collections, [Collection { name: "Nova".into(), collapsed: false }]);
        assert_eq!(cfg.slots[0].collection, None);
        assert_eq!(cfg.slots[1].key, "");
        assert_eq!(view(&cfg), [v(None, &["x"]), v(Some("Nova"), &["y"])]);
    }

    #[test]
    fn normalize_drops_duplicate_and_blank_collections() {
        let mut cfg: Config =
            serde_json::from_str(r#"{"collections": ["A", {"name": "A"}, {"name": " "}, true], "slots": []}"#).unwrap();
        normalize(&mut cfg);
        assert_eq!(names(&cfg), ["A"]);
    }
}
