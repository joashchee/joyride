// Reference kit: locomotion loops. Subjects come from docs/locomotion-reference-kit.md; the gaps are the work list.
export default {
  id: 'locomotion',
  title: 'Locomotion loops',
  doc: 'docs/locomotion-reference-kit.md',
  subjects: [
    { id: 'walk', label: 'Walk cycle (contact, passing)', recipes: [{ recipe: 'walk', params: {} }, { recipe: 'walk', params: { frames: 8 } }] },
    { id: 'run', label: 'Run cycle (contact, flight)', recipes: [{ recipe: 'run', params: {} }, { recipe: 'run', params: { frames: 8 } }] },
    { id: 'jump', label: 'Jump in place (crouch, takeoff, apex, fall, land)', recipes: [{ recipe: 'jump', params: {} }, { recipe: 'jump', params: { frames: 8 } }] },
    { id: 'idle', label: 'Idle breathing', gap: 'Painter has idle(); an Animator idle with weight shift is next' },
    { id: 'jump-travel', label: 'Jump with forward travel (root motion)', gap: 'dx over the air frames; needs the root-motion rule in the plan' },
    { id: 'sneak-crouch', label: 'Sneak and crouch-walk', gap: 'walk with squat held; a stride and bounce preset' },
    { id: 'fall-land-roll', label: 'Fall, land and roll', gap: 'a jump variant with a long fall and a roll' },
    { id: 'climb-swim', label: 'Climb and swim', gap: 'arm-led loops; needs more arm values from Painter' },
    { id: 'dash-slide', label: 'Dash and slide', gap: 'a few-frame burst with a hold on the stretch' },
    { id: 'mocap-rotoscope', label: 'Motion capture, rotoscoped footage, ripped sprite sheets', reference: 'study only: never traced, never a fixture' },
  ],
};
