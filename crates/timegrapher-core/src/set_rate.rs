//! Where a set beat rate came from (#2224): one rule for every document
//! that gives `bph_set`.

use crate::take::SettingFrom;

/// Where a set beat rate came from (`bph_set_from`, Max on #2224): given
/// for this reading as one of [`crate::beats::STANDARD_BPH`] or typed as
/// any other, the calibre's table row, or the take's session file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BphFrom {
    Standard,
    Typed,
    Calibre,
    Session,
}

impl BphFrom {
    /// Where the set rate came from, by [`SettingFrom::of`]: `given` for
    /// this reading, else what a session file `kept`, else a calibre's
    /// `table` rate. `None` when none sets it, so the rate is heard (Auto).
    pub fn of(given: Option<u32>, kept: Option<u32>, table: Option<u32>) -> Option<Self> {
        match SettingFrom::of(given, kept, table) {
            SettingFrom::Given => Some(match given {
                Some(b) if crate::beats::STANDARD_BPH.contains(&b) => Self::Standard,
                _ => Self::Typed,
            }),
            SettingFrom::Take => Some(Self::Session),
            SettingFrom::Calibre => Some(Self::Calibre),
            SettingFrom::Default => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Manifest;
    use crate::take::{stream_settings, Calibre, Given};

    /// Where a set rate came from: given (a standard rate or a typed
    /// one), the session file, the calibre, or none on Auto (#2224).
    #[test]
    fn a_set_rate_says_where_it_came_from() {
        use BphFrom::{Session, Standard, Typed};
        assert_eq!(BphFrom::of(Some(28_800), None, None), Some(Standard));
        assert_eq!(BphFrom::of(Some(17_570), None, None), Some(Typed));
        assert_eq!(
            BphFrom::of(Some(17_570), Some(18_000), Some(21_600)),
            Some(Typed)
        );
        assert_eq!(
            BphFrom::of(Some(18_000), Some(18_000), None),
            Some(Standard)
        );
        assert_eq!(BphFrom::of(None, Some(17_570), Some(21_600)), Some(Session));
        assert_eq!(
            BphFrom::of(None, None, Some(21_600)),
            Some(BphFrom::Calibre)
        );
        assert_eq!(BphFrom::of(None, None, None), None);
        for b in crate::beats::STANDARD_BPH {
            assert_eq!(BphFrom::of(Some(b), None, None), Some(Standard), "{b}");
        }
        let m: Manifest = toml::from_str("bph = 17570").unwrap();
        let cal = |bph| Calibre {
            name: "X".into(),
            bph,
            lift_angle_deg: None,
            lift_set_aside_deg: None,
            escape_teeth: None,
            escapement: None,
            wheels: Vec::new(),
            meshes: Vec::new(),
        };
        let set = |m: &Manifest, c: Option<&Calibre>, bph| {
            let a = stream_settings(
                m,
                c,
                Given {
                    bph,
                    lift_deg: None,
                },
            )
            .analysis;
            (a.bph, a.bph_from)
        };
        assert_eq!(set(&m, None, None), (Some(17_570), Some(Session)));
        assert_eq!(set(&m, None, Some(28_800)), (Some(28_800), Some(Standard)));
        let none = Manifest::default();
        assert_eq!(
            set(&none, Some(&cal(Some(21_600))), None),
            (Some(21_600), Some(BphFrom::Calibre))
        );
        assert_eq!(set(&none, Some(&cal(None)), None), (None, None));
    }
}
