const HEAD = `<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg
  width="512"
  height="512"
  viewBox="0 0 512 512"
  version="1.1"
  xmlns="http://www.w3.org/2000/svg"
  xmlns:svg="http://www.w3.org/2000/svg">
`;
const FOOT = `</svg>
`;

const FACE = `<ellipse
  style="display:inline;fill:#42ff42;fill-opacity:1;stroke:#000000;stroke-width:23.1498;stroke-linecap:round;stroke-opacity:1"
  id="path358"
  cx="262.65161"
  cy="256"
  rx="159.45966"
  ry="224.60014" />
`;

const SKIN_COL = ["413a22", "867746", "866346", "868046", "c3b68b", "e2dcc8"];

const EYES = `<ellipse
   style="fill:#42ff42;fill-opacity:1;stroke:#000000;stroke-width:10.01574803;stroke-linecap:round;stroke-opacity:1;stroke-dasharray:none"
   id="path1509"
   cx="207.81587"
   cy="191.11792"
   rx="38.721123"
   ry="40.649834" />
<ellipse
   style="fill:#42ff42;fill-opacity:1;stroke:#000000;stroke-width:10.0157;stroke-linecap:round;stroke-dasharray:none;stroke-opacity:1"
   id="ellipse1563"
   cx="321.88858"
   cy="191.11792"
   rx="38.721123"
   ry="40.649834" />
<ellipse
   style="fill:#000000;fill-opacity:1;stroke:none;stroke-width:10.0157;stroke-linecap:round;stroke-dasharray:none;stroke-opacity:1"
   id="path1565"
   cx="217.8418"
   cy="196.862"
   rx="13.833457"
   ry="14.219672" />
<ellipse
   style="fill:#000000;fill-opacity:1;stroke:none;stroke-width:10.0157;stroke-linecap:round;stroke-dasharray:none;stroke-opacity:1"
   id="path1565-3"
   cx="316.23236"
   cy="196.862"
   rx="13.833457"
   ry="14.219672" />`;

const EYE_COL = ["66ff66", "6666ff", "66ffff", "ffff66", "666600"];

export function avatarSvg(code: Uint8Array): string {
  const face = FACE.replaceAll("42ff42", SKIN_COL[code[1] % SKIN_COL.length]);
  const eyes = EYES.replaceAll("42ff42", EYE_COL[code[2] % EYE_COL.length]);
  return `${HEAD}${face}${eyes}${FOOT}`;
}
