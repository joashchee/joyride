// The engine's version, as data the main entry can hold (it cannot read package.json). A test keeps it equal to package.json, so a
// checkpoint that bumps one bumps both. Packs, `resolveLook` and the export record carry it: the same params draw the same pixels
// within one version, and this says which one.
export const ENGINE_VERSION = '1.31.1';
