// Icon set used by the workbench chrome. Every drawing is a 24x24 stroked path in the same style as
// `design/camcraft-workbench-v2/02-floating-studio.html`, so no icon font or external file is loaded.
// Shapes are data, not logic: the `Icon` atom just renders them.
//
// 095 T5 (L8d): the artboard draws no save action, so `save` (floppy) and `save-as` (floppy with a
// pen on its open right side) are drawn here for the two TopBar file actions that were sharing the
// `import` glyph. They are distinct drawings, not a relabelled copy of the same shape.
export type IconName =
  | 'arrow-right'
  | 'folder'
  | 'import'
  | 'layers'
  | 'origin'
  | 'plus'
  | 'save'
  | 'save-as'
  | 'search'
  | 'settings'
  | 'theme';

type PathShape = { tag: 'path'; d: string };
type CircleShape = { tag: 'circle'; cx: number; cy: number; r: number };
type RectShape = {
  tag: 'rect';
  x: number;
  y: number;
  w: number;
  h: number;
  rx?: number;
};

export type IconShape = PathShape | CircleShape | RectShape;

export const ICONS: Record<IconName, IconShape[]> = {
  'arrow-right': [{ tag: 'path', d: 'M5 12h14M13 6l6 6-6 6' }],
  folder: [
    {
      tag: 'path',
      d: 'M4 7h6l2 2h8v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z'
    }
  ],
  import: [{ tag: 'path', d: 'M12 3v12M8 11l4 4 4-4M4 19h16' }],
  layers: [
    { tag: 'rect', x: 4, y: 4, w: 16, h: 16, rx: 2 },
    { tag: 'path', d: 'M4 9h16M9 4v16' }
  ],
  origin: [
    { tag: 'circle', cx: 12, cy: 12, r: 7 },
    { tag: 'path', d: 'M12 5v2M12 17v2M5 12h2M17 12h2' }
  ],
  plus: [{ tag: 'path', d: 'M12 5v14M5 12h14' }],
  save: [
    {
      tag: 'path',
      d: 'M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z'
    },
    { tag: 'path', d: 'M17 21v-8H7v8' },
    { tag: 'path', d: 'M7 3v5h8' }
  ],
  'save-as': [
    // Floppy open on its right side, where the pen writes: the file goes to a new path.
    {
      tag: 'path',
      d: 'M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h8l4 4v2'
    },
    { tag: 'path', d: 'M7 3v5h5' },
    {
      tag: 'path',
      d: 'M20.8 11.2a1.7 1.7 0 0 0-2.4 0l-5 5-.6 3 3-.6 5-5a1.7 1.7 0 0 0 0-2.4z'
    }
  ],
  search: [
    { tag: 'circle', cx: 11, cy: 11, r: 6 },
    { tag: 'path', d: 'M20 20l-4.2-4.2' }
  ],
  settings: [
    { tag: 'circle', cx: 12, cy: 12, r: 3 },
    {
      tag: 'path',
      d: 'M19 12a7 7 0 0 0-.1-1l2-1.5-1.8-3.1-2.3.8a7 7 0 0 0-1.7-1l-.3-2.4h-3.6l-.3 2.4a7 7 0 0 0-1.7 1l-2.3-.8L4.1 9.5 6.1 11a7 7 0 0 0 0 2l-2 1.5 1.8 3.1 2.3-.8a7 7 0 0 0 1.7 1l.3 2.4h3.6l.3-2.4a7 7 0 0 0 1.7-1l2.3.8 1.8-3.1-2-1.5c.1-.3.1-.7.1-1z'
    }
  ],
  theme: [{ tag: 'path', d: 'M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z' }]
};
