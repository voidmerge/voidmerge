import assets from "./assets.js";

function getPart(name: string, color: number): string {
  const part = assets[`avatar-${name}.svg-part`] || "";
  const colorVal = COLOR[color % COLOR.length];
  return part.replaceAll("42ff42", colorVal);
}

const NOSES = ["a", "b"];
const HAIR = ["a", "b"];
const EYES = ["a", "b"];
const MOUTH = ["a", "b"];

const COLOR = [
  "413a22",
  "867746",
  "866346",
  "868046",
  "c3b68b",
  "e2dcc8",
  "66ff66",
  "6666ff",
  "66ffff",
  "ffff66",
  "666600",
];

export function avatarSvg(code: Uint8Array): string {
  const face = getPart("face", code[0]);
  const noseId = NOSES[code[1] % NOSES.length];
  const nose = getPart(`nose-${noseId}`, code[2]);
  const hairId = HAIR[code[3] % HAIR.length];
  const hair = getPart(`hair-${hairId}`, code[4]);
  const eyesId = EYES[code[5] % EYES.length];
  const eyes = getPart(`eyes-${eyesId}`, code[6]);
  const mouthId = MOUTH[code[7] % MOUTH.length];
  const mouth = getPart(`mouth-${mouthId}`, code[8]);
  return `${HEAD}${face}${nose}${hair}${eyes}${mouth}${FOOT}`;
}

// -- let these hoist... since the xml header messes vim up -- //

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
