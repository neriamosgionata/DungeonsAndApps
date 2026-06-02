export type ItemCategory = 'weapon' | 'armor' | 'shield' | 'adventuring_gear' | 'ammunition' | 'pack';

export type WeaponProperty = 'light' | 'finesse' | 'heavy' | 'reach' | 'two-handed' | 'versatile' | 'thrown' | 'ranged' | 'loading' | 'ammunition' | 'special';

export type ArmorType = 'light' | 'medium' | 'heavy' | 'shield';

export interface ItemDef {
  slug: string;
  name: string;
  category: ItemCategory;
  cost_gp: number;
  weight_lb: number;
  armor_type?: ArmorType;
  ac_base?: number;
  max_dex?: number;
  stealth_disadvantage?: boolean;
  str_requirement?: number;
  damage_die?: string;
  damage_type?: string;
  properties?: WeaponProperty[];
  range_normal?: number;
  range_long?: number;
  versatile_die?: string;
  description?: string;
}

export const ITEMS: ItemDef[] = [
  // ---- Light Armor ----
  { slug: 'padded', name: 'Padded', category: 'armor', cost_gp: 5, weight_lb: 8, armor_type: 'light', ac_base: 11, max_dex: 99, stealth_disadvantage: true },
  { slug: 'leather', name: 'Leather', category: 'armor', cost_gp: 10, weight_lb: 10, armor_type: 'light', ac_base: 11, max_dex: 99 },
  { slug: 'studded-leather', name: 'Studded Leather', category: 'armor', cost_gp: 45, weight_lb: 13, armor_type: 'light', ac_base: 12, max_dex: 99 },
  // ---- Medium Armor ----
  { slug: 'hide', name: 'Hide', category: 'armor', cost_gp: 10, weight_lb: 12, armor_type: 'medium', ac_base: 12, max_dex: 2 },
  { slug: 'chain-shirt', name: 'Chain Shirt', category: 'armor', cost_gp: 50, weight_lb: 20, armor_type: 'medium', ac_base: 13, max_dex: 2 },
  { slug: 'scale-mail', name: 'Scale Mail', category: 'armor', cost_gp: 50, weight_lb: 45, armor_type: 'medium', ac_base: 14, max_dex: 2, stealth_disadvantage: true },
  { slug: 'breastplate', name: 'Breastplate', category: 'armor', cost_gp: 400, weight_lb: 20, armor_type: 'medium', ac_base: 14, max_dex: 2 },
  { slug: 'half-plate', name: 'Half Plate', category: 'armor', cost_gp: 750, weight_lb: 40, armor_type: 'medium', ac_base: 15, max_dex: 2, stealth_disadvantage: true, str_requirement: 15 },
  // ---- Heavy Armor ----
  { slug: 'ring-mail', name: 'Ring Mail', category: 'armor', cost_gp: 30, weight_lb: 40, armor_type: 'heavy', ac_base: 14, max_dex: 0, stealth_disadvantage: true, str_requirement: 13 },
  { slug: 'chain-mail', name: 'Chain Mail', category: 'armor', cost_gp: 75, weight_lb: 55, armor_type: 'heavy', ac_base: 16, max_dex: 0, stealth_disadvantage: true, str_requirement: 13 },
  { slug: 'splint', name: 'Splint', category: 'armor', cost_gp: 200, weight_lb: 60, armor_type: 'heavy', ac_base: 17, max_dex: 0, stealth_disadvantage: true, str_requirement: 15 },
  { slug: 'plate', name: 'Plate', category: 'armor', cost_gp: 1500, weight_lb: 65, armor_type: 'heavy', ac_base: 18, max_dex: 0, stealth_disadvantage: true, str_requirement: 15 },
  // ---- Shields ----
  { slug: 'shield', name: 'Shield', category: 'shield', cost_gp: 10, weight_lb: 6, ac_base: 2 },
  // ---- Simple Melee Weapons ----
  { slug: 'club', name: 'Club', category: 'weapon', cost_gp: 0.1, weight_lb: 2, damage_die: '1d4', damage_type: 'bludgeoning', properties: ['light'] },
  { slug: 'dagger', name: 'Dagger', category: 'weapon', cost_gp: 2, weight_lb: 1, damage_die: '1d4', damage_type: 'piercing', properties: ['finesse', 'light', 'thrown'], range_normal: 20, range_long: 60 },
  { slug: 'greatclub', name: 'Greatclub', category: 'weapon', cost_gp: 0.2, weight_lb: 10, damage_die: '1d8', damage_type: 'bludgeoning', properties: ['two-handed'] },
  { slug: 'handaxe', name: 'Handaxe', category: 'weapon', cost_gp: 5, weight_lb: 2, damage_die: '1d6', damage_type: 'slashing', properties: ['light', 'thrown'], range_normal: 20, range_long: 60 },
  { slug: 'javelin', name: 'Javelin', category: 'weapon', cost_gp: 0.5, weight_lb: 2, damage_die: '1d6', damage_type: 'piercing', properties: ['thrown'], range_normal: 30, range_long: 120 },
  { slug: 'light-hammer', name: 'Light Hammer', category: 'weapon', cost_gp: 2, weight_lb: 2, damage_die: '1d4', damage_type: 'bludgeoning', properties: ['light', 'thrown'], range_normal: 20, range_long: 60 },
  { slug: 'mace', name: 'Mace', category: 'weapon', cost_gp: 5, weight_lb: 4, damage_die: '1d6', damage_type: 'bludgeoning', properties: [] },
  { slug: 'quarterstaff', name: 'Quarterstaff', category: 'weapon', cost_gp: 0.2, weight_lb: 4, damage_die: '1d6', damage_type: 'bludgeoning', properties: ['versatile'], versatile_die: '1d8' },
  { slug: 'sickle', name: 'Sickle', category: 'weapon', cost_gp: 1, weight_lb: 2, damage_die: '1d4', damage_type: 'slashing', properties: ['light'] },
  { slug: 'spear', name: 'Spear', category: 'weapon', cost_gp: 1, weight_lb: 3, damage_die: '1d6', damage_type: 'piercing', properties: ['thrown', 'versatile'], range_normal: 20, range_long: 60, versatile_die: '1d8' },
  // ---- Simple Ranged Weapons ----
  { slug: 'light-crossbow', name: 'Light Crossbow', category: 'weapon', cost_gp: 25, weight_lb: 5, damage_die: '1d8', damage_type: 'piercing', properties: ['ranged', 'loading', 'two-handed', 'ammunition'], range_normal: 80, range_long: 320 },
  { slug: 'shortbow', name: 'Shortbow', category: 'weapon', cost_gp: 25, weight_lb: 2, damage_die: '1d6', damage_type: 'piercing', properties: ['ranged', 'two-handed', 'ammunition'], range_normal: 80, range_long: 320 },
  { slug: 'sling', name: 'Sling', category: 'weapon', cost_gp: 0.1, weight_lb: 0, damage_die: '1d4', damage_type: 'bludgeoning', properties: ['ranged', 'ammunition'], range_normal: 30, range_long: 120 },
  // ---- Martial Melee Weapons ----
  { slug: 'battleaxe', name: 'Battleaxe', category: 'weapon', cost_gp: 10, weight_lb: 4, damage_die: '1d8', damage_type: 'slashing', properties: ['versatile'], versatile_die: '1d10' },
  { slug: 'flail', name: 'Flail', category: 'weapon', cost_gp: 10, weight_lb: 2, damage_die: '1d8', damage_type: 'bludgeoning', properties: [] },
  { slug: 'glaive', name: 'Glaive', category: 'weapon', cost_gp: 20, weight_lb: 6, damage_die: '1d10', damage_type: 'slashing', properties: ['heavy', 'reach', 'two-handed'] },
  { slug: 'greataxe', name: 'Greataxe', category: 'weapon', cost_gp: 30, weight_lb: 7, damage_die: '1d12', damage_type: 'slashing', properties: ['heavy', 'two-handed'] },
  { slug: 'greatsword', name: 'Greatsword', category: 'weapon', cost_gp: 50, weight_lb: 6, damage_die: '2d6', damage_type: 'slashing', properties: ['heavy', 'two-handed'] },
  { slug: 'halberd', name: 'Halberd', category: 'weapon', cost_gp: 20, weight_lb: 6, damage_die: '1d10', damage_type: 'slashing', properties: ['heavy', 'reach', 'two-handed'] },
  { slug: 'lance', name: 'Lance', category: 'weapon', cost_gp: 10, weight_lb: 6, damage_die: '1d12', damage_type: 'piercing', properties: ['reach', 'special'] },
  { slug: 'longsword', name: 'Longsword', category: 'weapon', cost_gp: 15, weight_lb: 3, damage_die: '1d8', damage_type: 'slashing', properties: ['versatile'], versatile_die: '1d10' },
  { slug: 'maul', name: 'Maul', category: 'weapon', cost_gp: 10, weight_lb: 10, damage_die: '2d6', damage_type: 'bludgeoning', properties: ['heavy', 'two-handed'] },
  { slug: 'morningstar', name: 'Morningstar', category: 'weapon', cost_gp: 15, weight_lb: 4, damage_die: '1d8', damage_type: 'piercing', properties: [] },
  { slug: 'pike', name: 'Pike', category: 'weapon', cost_gp: 5, weight_lb: 18, damage_die: '1d10', damage_type: 'piercing', properties: ['heavy', 'reach', 'two-handed'] },
  { slug: 'rapier', name: 'Rapier', category: 'weapon', cost_gp: 25, weight_lb: 2, damage_die: '1d8', damage_type: 'piercing', properties: ['finesse'] },
  { slug: 'scimitar', name: 'Scimitar', category: 'weapon', cost_gp: 25, weight_lb: 3, damage_die: '1d6', damage_type: 'slashing', properties: ['finesse', 'light'] },
  { slug: 'shortsword', name: 'Shortsword', category: 'weapon', cost_gp: 10, weight_lb: 2, damage_die: '1d6', damage_type: 'piercing', properties: ['finesse', 'light'] },
  { slug: 'trident', name: 'Trident', category: 'weapon', cost_gp: 5, weight_lb: 4, damage_die: '1d6', damage_type: 'piercing', properties: ['thrown', 'versatile'], range_normal: 20, range_long: 60, versatile_die: '1d8' },
  { slug: 'war-pick', name: 'War Pick', category: 'weapon', cost_gp: 5, weight_lb: 2, damage_die: '1d8', damage_type: 'piercing', properties: [] },
  { slug: 'warhammer', name: 'Warhammer', category: 'weapon', cost_gp: 15, weight_lb: 2, damage_die: '1d8', damage_type: 'bludgeoning', properties: ['versatile'], versatile_die: '1d10' },
  { slug: 'whip', name: 'Whip', category: 'weapon', cost_gp: 2, weight_lb: 3, damage_die: '1d4', damage_type: 'slashing', properties: ['finesse', 'reach'] },
  // ---- Martial Ranged Weapons ----
  { slug: 'blowgun', name: 'Blowgun', category: 'weapon', cost_gp: 10, weight_lb: 1, damage_die: '1', damage_type: 'piercing', properties: ['ranged', 'loading', 'ammunition'], range_normal: 25, range_long: 100 },
  { slug: 'hand-crossbow', name: 'Hand Crossbow', category: 'weapon', cost_gp: 75, weight_lb: 3, damage_die: '1d6', damage_type: 'piercing', properties: ['ranged', 'light', 'loading', 'ammunition'], range_normal: 30, range_long: 120 },
  { slug: 'heavy-crossbow', name: 'Heavy Crossbow', category: 'weapon', cost_gp: 50, weight_lb: 18, damage_die: '1d10', damage_type: 'piercing', properties: ['ranged', 'heavy', 'loading', 'two-handed', 'ammunition'], range_normal: 100, range_long: 400 },
  { slug: 'longbow', name: 'Longbow', category: 'weapon', cost_gp: 50, weight_lb: 2, damage_die: '1d8', damage_type: 'piercing', properties: ['ranged', 'heavy', 'two-handed', 'ammunition'], range_normal: 150, range_long: 600 },
  // ---- Adventuring Gear ----
  { slug: 'backpack', name: 'Backpack', category: 'adventuring_gear', cost_gp: 2, weight_lb: 5 },
  { slug: 'candle', name: 'Candle', category: 'adventuring_gear', cost_gp: 0.01, weight_lb: 0 },
  { slug: 'crowbar', name: 'Crowbar', category: 'adventuring_gear', cost_gp: 2, weight_lb: 5 },
  { slug: 'healers-kit', name: "Healer's Kit", category: 'adventuring_gear', cost_gp: 5, weight_lb: 3 },
  { slug: 'holy-water', name: 'Holy Water (flask)', category: 'adventuring_gear', cost_gp: 25, weight_lb: 1 },
  { slug: 'lantern-hooded', name: 'Lantern (Hooded)', category: 'adventuring_gear', cost_gp: 5, weight_lb: 2 },
  { slug: 'rope-hemp', name: 'Rope (Hemp, 50 ft)', category: 'adventuring_gear', cost_gp: 1, weight_lb: 10 },
  { slug: 'torch', name: 'Torch', category: 'adventuring_gear', cost_gp: 0.01, weight_lb: 1 },
];

export function itemBySlug(slug: string): ItemDef | undefined {
  return ITEMS.find(i => i.slug === slug);
}

export function itemsByCategory(cat: ItemCategory): ItemDef[] {
  return ITEMS.filter(i => i.category === cat);
}
