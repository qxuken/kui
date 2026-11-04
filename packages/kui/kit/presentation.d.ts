import type { KuiNode, Ctx, KuiWindow, WindowLoop } from '../index.js';
import type { SettingsProps, ContentTheme, ContentLayout, ContentProps } from './components.js';

export interface SlideContext<S = any, A = any> {
  state: S;
  step: number;
  theme: ContentTheme;
  layout: ContentLayout;
  mode: 'live' | 'snapshot';
  size: { width: number; height: number };
  /** Milliseconds: live clock or the fixed snapshot timestamp. */
  now: number;
  msg<const T extends A>(message: T): T & { presentation: 'demo'; slide: string; generation: number; mode: 'live' | 'snapshot'; message: T };
  dispatch(message: A): void;
  reveal(step: number, children: KuiNode | KuiNode[], options?: { key?: string; collapse?: boolean }): KuiNode;
  Reveal(props: { step: number; key?: string; collapse?: boolean; children?: KuiNode | KuiNode[] }): KuiNode;
  content(props?: ContentProps, children?: KuiNode | KuiNode[]): KuiNode;
  Content(props: ContentProps & { children?: KuiNode | KuiNode[] }): KuiNode;
  /** Activation only. Automatically canceled on navigation/reload/shutdown. */
  every(ms: number, message: A | (() => A)): () => void;
  after(ms: number, message: A | (() => A)): () => void;
  spawn(task: (signal: AbortSignal) => Promise<A | void>): Promise<void>;
}
export interface Slide<S = any, A = any> {
  key: string;
  title?: string;
  notes?: string;
  init?: () => S;
  update?: (state: S, message: A) => S | void;
  activate?: (context: SlideContext<S, A>) => (() => void) | void;
  view: (context: SlideContext<S, A>) => KuiNode;
  /** Replacement for content that cannot render safely in snapshot mode. */
  snapshot?: (context: SlideContext<S, A>) => KuiNode;
  /** Required with a replacement snapshot; otherwise derived from reveals. */
  steps?: number;
  stateVersion?: string | number;
}
export interface Deck {
  title?: string;
  slides: Slide[];
  preset?: ContentTheme;
  theme?: ContentTheme;
  layout?: ContentLayout;
  settings?: SettingsProps;
  /** Font file paths; family names in settings select the registered faces. */
  fonts?: string[];
  presenterShortcut?: { key: string; primary?: boolean; ctrl?: boolean; super?: boolean; alt?: boolean; shift?: boolean };
  canvas?: { w: number; h: number } | { width: number; height: number };
}
export interface PresentationOptions {
  presenter?: boolean;
  /** Explicitly embed the presenter's toolbar in custom window chrome. Default false. */
  toolbarInTitlebar?: boolean;
  size?: { width: number; height: number };
  clock?: () => number;
  dispatch?: (message: any) => void;
  every?: (callback: () => void, ms: number) => () => void;
  after?: (callback: () => void, ms: number) => () => void;
  onError?: (error: unknown) => void;
  snapshot?: boolean;
  snapshotTime?: number;
}
export interface PresentationModel {
  /** Audience position; the private presenter selection is separate while liveSync is false. */
  index: number;
  key: string;
  step: number;
  states: Record<string, any>;
  versions: Record<string, string | number>;
  theme: ContentTheme;
  layout: ContentLayout;
  presenter: boolean;
  overview: boolean;
  help: boolean;
  confirmRestart: boolean;
  display: string;
  elapsed: number;
  running: boolean;
  liveSync: boolean;
  preview: { index: number; key: string; step: number };
}
export interface ExportFrame { key: string; title: string; index: number; step: number; image?: string; timestamp_ms?: number }
export class Presentation {
  constructor(deck: Deck, options?: PresentationOptions, previous?: Presentation);
  deck: Deck;
  model: PresentationModel;
  counts: Record<string, number>;
  options: PresentationOptions;
  start(): void;
  dispose(): void;
  send(message: any): any;
  update(message: any): PresentationModel;
  position(source?: 'presenter' | 'audience'): { index: number; key: string; step: number };
  presentSlide(): void;
  next(source?: 'presenter' | 'audience'): void;
  previous(source?: 'presenter' | 'audience'): void;
  jump(key: string, source?: 'presenter' | 'audience'): void;
  reload(deck: Deck): PresentationModel;
  elapsed(): number;
  context(index: number, step: number, mode: 'live' | 'snapshot', size?: { width: number; height: number }, revealGuide?: boolean): SlideContext;
  renderSlide(index: number, step: number, mode?: 'live' | 'snapshot', size?: { width: number; height: number }, revealGuide?: boolean): KuiNode;
  view(surface: Ctx | KuiWindow, window?: string): KuiNode;
  exportPlan(allSteps?: boolean): ExportFrame[];
}
export function createPresentation(deck: Deck, options?: PresentationOptions): Presentation;
export interface PresentOptions extends PresentationOptions {
  args?: string[];
  watch?: boolean;
  export?: string;
  width?: number;
  height?: number;
  /** Export raster scale; live windows use the display's scale. */
  scale?: number;
  appearance?: 'light' | 'dark';
  chrome?: 'native' | 'custom' | 'borderless';
  titlebar?: 'standard' | 'medium' | 'tall';
  extensions?: Array<{ path: string; namespace?: string }>;
  setup?: (app: WindowLoop<PresentationModel, any>, controller: Presentation) => void;
}
export function present(source: Deck | string | URL, options?: PresentOptions): Promise<any>;
export function exportPresentation(deck: Deck, options?: {
  directory?: string; size?: { width: number; height: number }; scale?: number;
  appearance?: 'light' | 'dark'; allSteps?: boolean; timestamp?: number;
}): Promise<{ version: number; width: number; height: number; scale: number; appearance: string; frames: ExportFrame[] }>;
