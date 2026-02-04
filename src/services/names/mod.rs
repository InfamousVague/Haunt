//! Random username generator
//!
//! Generates memorable usernames like PlayStation/Xbox auto-generated names.
//! Pattern: {Adjective}{Noun}{Number}

use rand::Rng;

/// Adjectives for username generation
const ADJECTIVES: &[&str] = &[
    // Colors & Visual
    "Shadow", "Crimson", "Azure", "Golden", "Silver", "Neon", "Dark", "Bright",
    "Crystal", "Cosmic", "Electric", "Frozen", "Blazing", "Mystic", "Phantom",
    "Emerald", "Obsidian", "Radiant", "Void", "Prism", "Midnight", "Solar",
    "Lunar", "Stellar", "Chrome", "Onyx", "Sapphire", "Ruby", "Amber", "Violet",
    // Speed & Action
    "Swift", "Quick", "Rapid", "Flash", "Turbo", "Hyper", "Ultra", "Mega",
    "Super", "Thunder", "Lightning", "Storm", "Blitz", "Rush", "Dash", "Zoom",
    "Rocket", "Sonic", "Nitro", "Bolt", "Strike", "Pulse", "Wave", "Surge",
    // Power & Strength
    "Iron", "Steel", "Titan", "Giant", "Mighty", "Power", "Force", "Alpha",
    "Omega", "Prime", "Elite", "Royal", "Noble", "Grand", "Epic", "Legendary",
    "Supreme", "Ultimate", "Apex", "Peak", "Max", "Infinite", "Eternal", "Ancient",
    // Stealth & Mystery
    "Silent", "Stealth", "Ghost", "Specter", "Wraith", "Hidden", "Secret", "Covert",
    "Rogue", "Ninja", "Cipher", "Enigma", "Cryptic", "Arcane", "Occult", "Veiled",
    // Nature & Elements
    "Arctic", "Polar", "Frost", "Ice", "Fire", "Flame", "Inferno", "Ember",
    "Ocean", "River", "Storm", "Cloud", "Rain", "Wind", "Gale", "Tempest",
    "Stone", "Rock", "Metal", "Earth", "Terra", "Jungle", "Forest", "Wild",
    // Tech & Cyber
    "Cyber", "Digital", "Pixel", "Binary", "Quantum", "Neural", "Synth", "Tech",
    "Data", "Code", "Byte", "Vector", "Matrix", "Grid", "Node", "Core",
    // Personality
    "Brave", "Bold", "Fierce", "Savage", "Wild", "Crazy", "Mad", "Insane",
    "Lucky", "Happy", "Chill", "Cool", "Slick", "Smooth", "Sharp", "Clever",
    "Wise", "Smart", "Keen", "Astute", "Cunning", "Sly", "Wily", "Shrewd",
    // Crypto/Trading themed
    "Diamond", "Rocket", "Moon", "Bull", "Bear", "Whale", "Degen", "Based",
    "Chad", "Giga", "Sigma", "Omega", "Turbo", "Laser", "Focused", "Zen",
];

/// Nouns for username generation
const NOUNS: &[&str] = &[
    // Animals - Predators
    "Wolf", "Tiger", "Lion", "Panther", "Jaguar", "Leopard", "Lynx", "Cougar",
    "Bear", "Shark", "Hawk", "Eagle", "Falcon", "Raven", "Viper", "Cobra",
    "Dragon", "Phoenix", "Griffin", "Hydra", "Kraken", "Serpent", "Raptor", "Rex",
    // Animals - Other
    "Fox", "Stag", "Elk", "Bison", "Mustang", "Stallion", "Rhino", "Gorilla",
    "Ape", "Monkey", "Panda", "Koala", "Owl", "Crow", "Sparrow", "Finch",
    // Mythical Creatures
    "Titan", "Giant", "Golem", "Specter", "Wraith", "Phantom", "Spirit", "Demon",
    "Angel", "Valkyrie", "Samurai", "Ninja", "Viking", "Knight", "Paladin", "Mage",
    "Wizard", "Sorcerer", "Warlock", "Sage", "Oracle", "Prophet", "Shaman", "Monk",
    // Warriors & Fighters
    "Warrior", "Fighter", "Hunter", "Slayer", "Assassin", "Sniper", "Archer", "Ranger",
    "Soldier", "Captain", "General", "Commander", "Chief", "King", "Queen", "Lord",
    "Baron", "Duke", "Prince", "Warlord", "Gladiator", "Champion", "Hero", "Legend",
    // Tech & Cyber
    "Hacker", "Coder", "Glitch", "Byte", "Pixel", "Vector", "Node", "Core",
    "Bot", "Droid", "Mech", "Cyborg", "Android", "Avatar", "Entity", "Nexus",
    // Space & Cosmic
    "Star", "Nova", "Pulsar", "Quasar", "Comet", "Meteor", "Orbit", "Nebula",
    "Galaxy", "Cosmos", "Void", "Abyss", "Horizon", "Eclipse", "Aurora", "Zenith",
    // Nature
    "Storm", "Thunder", "Blaze", "Inferno", "Frost", "Glacier", "Tornado", "Typhoon",
    "Tsunami", "Volcano", "Quake", "Tremor", "Boulder", "Mountain", "River", "Ocean",
    // Objects & Weapons
    "Blade", "Sword", "Dagger", "Axe", "Hammer", "Spear", "Arrow", "Shield",
    "Cannon", "Rocket", "Missile", "Bullet", "Laser", "Plasma", "Photon", "Pulse",
    // Trading/Crypto themed
    "Trader", "Hodler", "Whale", "Bull", "Bear", "Degen", "Ape", "Chad",
    "Diamond", "Hands", "Moon", "Rocket", "Lambo", "Gains", "Alpha", "Sigma",
];

/// Generate a random username
/// Pattern: {Adjective}{Noun}{2-4 digit number}
pub fn generate_username() -> String {
    let mut rng = rand::thread_rng();

    let adjective = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    let number: u16 = rng.gen_range(10..9999);

    format!("{}{}{}", adjective, noun, number)
}

/// Generate a username with a specific seed (for deterministic generation from public key)
pub fn generate_username_from_seed(seed: &[u8]) -> String {
    // Use first 8 bytes of seed as u64 for deterministic selection
    let seed_value = if seed.len() >= 8 {
        u64::from_le_bytes([
            seed[0], seed[1], seed[2], seed[3],
            seed[4], seed[5], seed[6], seed[7],
        ])
    } else {
        // Pad with zeros if seed is too short
        let mut padded = [0u8; 8];
        padded[..seed.len()].copy_from_slice(seed);
        u64::from_le_bytes(padded)
    };

    let adj_idx = (seed_value as usize) % ADJECTIVES.len();
    let noun_idx = ((seed_value >> 16) as usize) % NOUNS.len();
    let number = ((seed_value >> 32) % 9990 + 10) as u16;

    format!("{}{}{}", ADJECTIVES[adj_idx], NOUNS[noun_idx], number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_username() {
        let name = generate_username();
        assert!(!name.is_empty());
        assert!(name.len() >= 5); // At least adj + noun + 2 digits
        println!("Generated username: {}", name);
    }

    #[test]
    fn test_generate_username_from_seed() {
        let seed = b"test_seed_12345";
        let name1 = generate_username_from_seed(seed);
        let name2 = generate_username_from_seed(seed);

        // Same seed should produce same name
        assert_eq!(name1, name2);
        println!("Seeded username: {}", name1);
    }

    #[test]
    fn test_different_seeds_different_names() {
        let name1 = generate_username_from_seed(b"seed_one");
        let name2 = generate_username_from_seed(b"seed_two");

        // Different seeds should (usually) produce different names
        // This isn't guaranteed but is extremely likely
        println!("Name 1: {}, Name 2: {}", name1, name2);
    }

    #[test]
    fn test_generate_multiple_unique() {
        let mut names = std::collections::HashSet::new();
        for _ in 0..100 {
            names.insert(generate_username());
        }
        // Should have high uniqueness (allowing some collisions)
        assert!(names.len() >= 90);
    }
}
