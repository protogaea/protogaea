//! Automatic binomial names for clades (spec §7): the genus comes from the clade's dominant trait,
//! the epithet from its preferred habitat. A clade is named once, when it first reaches the naming
//! threshold, by how many clades of its combination were named before it; the name never changes.
//! Names are set by index, with no free text, so there is nothing to moderate; the dictionary is
//! short, neutral Latin, checked by hand for unwanted combinations. Not part of consensus.

use protogaea_core::Genome;

/// Genus roots by dominant trait, in the order M P G H D F.
const GENERA: [[&str; 8]; 6] = [
    [
        "Cursor",
        "Velox",
        "Celer",
        "Dromas",
        "Saltator",
        "Vagator",
        "Ambulator",
        "Migrans",
    ],
    [
        "Speculator",
        "Vigil",
        "Argus",
        "Oculatus",
        "Explorator",
        "Sentinel",
        "Auspex",
        "Lucidus",
    ],
    [
        "Pastor",
        "Folivor",
        "Graminus",
        "Carpophagus",
        "Messor",
        "Herbarius",
        "Frondator",
        "Ruminans",
    ],
    [
        "Venator",
        "Raptor",
        "Praedo",
        "Insidiator",
        "Sector",
        "Mandibulus",
        "Captor",
        "Latro",
    ],
    [
        "Loricatus",
        "Scutatus",
        "Testudo",
        "Armiger",
        "Clipeatus",
        "Munitus",
        "Cataphractus",
        "Tectus",
    ],
    [
        "Fecundus",
        "Genitor",
        "Nidifex",
        "Prolifer",
        "Ovifer",
        "Seminator",
        "Parens",
        "Fetosus",
    ],
];

/// Epithets by habitat: forest, steppe, desert, mountains, swamp, and generalists.
const EPITHETS: [[&str; 6]; 6] = [
    [
        "silvae",
        "nemoris",
        "silvestris",
        "frondosus",
        "umbrosus",
        "arborum",
    ],
    [
        "campi",
        "pratensis",
        "steppae",
        "herbosus",
        "planitiei",
        "ventosus",
    ],
    [
        "deserti",
        "arenae",
        "aridus",
        "solaris",
        "sabulosus",
        "siccus",
    ],
    [
        "montis",
        "alpinus",
        "rupium",
        "saxatilis",
        "cacuminis",
        "petraeus",
    ],
    [
        "paludis",
        "palustris",
        "limosus",
        "uliginis",
        "stagnalis",
        "ripae",
    ],
    [
        "vagus",
        "communis",
        "errans",
        "ubiquus",
        "varius",
        "mutabilis",
    ],
];

/// The combination a clade is named in: its dominant trait (ties go to the earlier trait) and its
/// preferred habitat, 6 × 6 of them.
pub fn combo(reference: &Genome) -> u32 {
    let trait_index = reference
        .traits
        .iter()
        .enumerate()
        .max_by_key(|&(i, &v)| (v, std::cmp::Reverse(i)))
        .map_or(0, |(i, _)| i);
    (trait_index * 6 + usize::from(reference.habitat.min(5))) as u32
}

/// The name of the `k`-th clade named in a combination, for example "Venator silvae". Once the 48
/// pairs of roots of a combination are used, a Roman numeral tells later clades apart.
pub fn name_in(combo: u32, k: u32) -> String {
    let (trait_index, habitat) = ((combo / 6) as usize % 6, (combo % 6) as usize);
    let genera = &GENERA[trait_index];
    let epithets = &EPITHETS[habitat];
    let k = k as usize;
    let genus = genera[k % genera.len()];
    let epithet = epithets[(k / genera.len()) % epithets.len()];
    let round = k / (genera.len() * epithets.len());
    if round == 0 {
        format!("{genus} {epithet}")
    } else {
        format!("{genus} {epithet} {}", roman(round + 1))
    }
}

fn roman(mut n: usize) -> String {
    const DIGITS: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, digit) in DIGITS {
        while n >= value {
            out.push_str(digit);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome(traits: [u8; 6], habitat: u8) -> Genome {
        Genome {
            traits,
            habitat,
            dispersal: 0,
            boldness: 0,
            hue: 0,
        }
    }

    #[test]
    fn names_follow_the_dominant_trait_and_habitat() {
        let hunter = combo(&genome([1, 1, 1, 8, 1, 1], 0));
        assert_eq!(name_in(hunter, 0), "Venator silvae");
        assert_eq!(name_in(hunter, 1), "Raptor silvae");
        assert_eq!(
            name_in(combo(&genome([8, 1, 1, 1, 1, 1], 2)), 8),
            "Cursor arenae"
        );
        assert_eq!(
            name_in(combo(&genome([1, 1, 1, 1, 8, 1], 3)), 0),
            "Loricatus montis"
        );
    }

    #[test]
    fn a_numeral_only_after_every_pair_is_used() {
        let grazer = combo(&genome([1, 1, 8, 1, 1, 1], 1));
        assert_eq!(name_in(grazer, 47), "Ruminans ventosus");
        assert_eq!(name_in(grazer, 48), "Pastor campi II");
        assert_eq!(name_in(grazer, 48 * 13), "Pastor campi XIV");
    }
}
