#!/usr/bin/env bun
// Transform Open5e /v1/spells/ dump → cinghialapp spells-srd.json.
// Open5e aggregates the SRD + open-licensed 5e expansions (Deep Magic,
// Tome of Heroes, Level Up A5E, Vault of Magic, etc.).
//
// Usage:
//   curl -sL 'https://api.open5e.com/v1/spells/?limit=2000' -o /tmp/open5e.json
//   bun shared/transform-open5e.ts /tmp/open5e.json

const input  = process.argv[2] ?? '/tmp/open5e-spells.json';
const output = process.argv[3] ?? new URL('./spells-srd.json', import.meta.url).pathname;

type Raw = {
  slug: string;
  name: string;
  desc: string;
  higher_level?: string | null;
  range: string;
  components: string;
  material?: string | null;
  ritual?: string;           // "yes" / "no"
  concentration?: string;    // "yes" / "no"
  requires_concentration?: boolean;
  can_be_cast_as_ritual?: boolean;
  duration: string;
  casting_time: string;
  level_int: number;
  school: string;
  dnd_class?: string;
  spell_lists?: string[];
  document__title?: string;
};

const raw = JSON.parse(await Bun.file(input).text()) as { count: number; results: Raw[] };

function classesOf(s: Raw): string[] {
  if (s.spell_lists && s.spell_lists.length) {
    return s.spell_lists
      .map((c) => c.trim())
      .filter(Boolean)
      .map((c) => c.charAt(0).toUpperCase() + c.slice(1));
  }
  if (!s.dnd_class) return [];
  return s.dnd_class
    .split(/,\s*/)
    .map((c) => c.replace(/\s*\(.+?\)\s*$/, '').trim())
    .filter(Boolean);
}

const seen = new Set<string>();
const spells = raw.results
  .map((s) => {
    const classes = classesOf(s);
    const mat = s.material ? ` (${s.material})` : '';
    const ritual = s.can_be_cast_as_ritual ?? s.ritual === 'yes';
    const concentration = s.requires_concentration ?? s.concentration === 'yes';
    return {
      slug: s.slug,
      name: s.name.trim(),
      level: s.level_int,
      school: (s.school ?? '').replace(/^./, (c) => c.toUpperCase()),
      casting_time: s.casting_time,
      range: s.range,
      components: s.components + mat,
      duration: s.duration,
      classes,
      ritual,
      concentration,
      description: (s.desc ?? '').trim(),
      higher_levels: s.higher_level?.trim() || null,
      source: s.document__title || 'Open5e',
    };
  })
  .filter((s) => {
    if (!s.slug || seen.has(s.slug)) return false;
    seen.add(s.slug);
    return s.level >= 0 && s.level <= 9 && s.name && s.description;
  })
  .sort((a, b) => a.level - b.level || a.name.localeCompare(b.name));

const out = {
  source: 'Open5e (SRD + open-licensed 5e expansions)',
  license: 'OGL 1.0a / CC-BY-4.0 per document — see document source',
  generated_at: new Date().toISOString(),
  count: spells.length,
  spells,
};

await Bun.write(output, JSON.stringify(out, null, 2));
console.log(`wrote ${out.count} spells to ${output}`);
