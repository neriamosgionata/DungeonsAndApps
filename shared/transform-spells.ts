#!/usr/bin/env bun
// transform 5e-bits/5e-database spell JSON → cinghialapp spells-srd.json

const input = process.argv[2] ?? '/tmp/srd-spells-raw.json';
const output = process.argv[3] ?? new URL('./spells-srd.json', import.meta.url).pathname;

type Raw = {
  index: string;
  name: string;
  desc: string[];
  higher_level?: string[];
  range: string;
  components: string[];
  material?: string;
  ritual: boolean;
  duration: string;
  concentration: boolean;
  casting_time: string;
  level: number;
  school: { name: string };
  classes: { name: string }[];
};

const raw: Raw[] = JSON.parse(await Bun.file(input).text());
const out = {
  source: 'SRD 5.1 (via 5e-bits/5e-database)',
  license: 'CC-BY-4.0',
  generated_at: new Date().toISOString(),
  count: raw.length,
  spells: raw.map((s) => ({
    slug: s.index,
    name: s.name,
    level: s.level,
    school: s.school.name,
    casting_time: s.casting_time,
    range: s.range,
    components: s.components.join(', ') + (s.material ? ` (${s.material})` : ''),
    duration: s.duration,
    classes: s.classes.map((c) => c.name),
    ritual: s.ritual,
    concentration: s.concentration,
    description: s.desc.join('\n\n'),
    higher_levels: s.higher_level?.join('\n\n') ?? null,
    source: 'SRD 5.1',
  })),
};

await Bun.write(output, JSON.stringify(out, null, 2));
console.log(`wrote ${out.count} spells to ${output}`);
