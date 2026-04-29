export type Language = 'en' | 'it';
export type UserRole = 'user' | 'admin';
export type MembershipRole = 'player' | 'master';
export type Visibility = 'private' | 'players' | 'public';

export interface User {
  id: string;
  email: string;
  display_name: string;
  role: UserRole;
  language: Language;
  avatar_url?: string | null;
}

export interface Campaign {
  id: string;
  name: string;
  description?: string | null;
  master_id: string;
  icon_url?: string | null;
  leveling?: 'xp' | 'milestone';
}

export interface Spell {
  slug: string;
  name: string;
  level: number;
  school: string;
  classes: string[];
  ritual: boolean;
  concentration: boolean;
  description: string;
  casting_time?: string | null;
  range_text?: string | null;
  components?: string | null;
  duration?: string | null;
  higher_levels?: string | null;
  source: string;
}

export interface DiceRollTerm {
  expr: string;
  kind: 'dice' | 'modifier';
  rolls: number[];
  kept: number[];
  value: number;
}

export interface DiceRollResult {
  id: string;
  expression: string;
  total: number;
  terms: DiceRollTerm[];
  label?: string | null;
}
