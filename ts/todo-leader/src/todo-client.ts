import { Ident } from "./ident.js";

const templates: { [name: string]: string } = {};

for (const s of document.querySelectorAll("[data-template]")) {
  if (s instanceof HTMLElement) {
    const name = s.dataset.template;
    if (typeof name === "string") {
      templates[name] = s.innerHTML;
    }
  }
}

function renderTemplate(
  name: string,
  replace?: { [k: string]: string },
): Element {
  let tpl = templates[name];
  console.log(tpl, replace);
  if (replace) {
    for (const k in replace) {
      tpl = tpl.replace(`{{${k}}}`, replace[k]);
    }
  }
  const d = document.createElement("div");
  d.innerHTML = tpl;
  const elem = d.removeChild(d.childNodes[0]);
  if (elem instanceof Element) {
    return elem;
  } else {
    return d;
  }
}

async function main() {
  const myIdent: Ident =
    (await Ident.load()) ||
    (await (async () => {
      const tmp = await Ident.random();
      tmp.store();
      return tmp;
    })());

  console.log("myIdent", myIdent.debug());

  // roll some random idents
  for (let i = 0; i < 5; ++i) {
    const ident = await Ident.random();
    const pick = renderTemplate("pick-ident", {
      short: ident.short(),
      ident: ident.ident(),
    });
    document.body.appendChild(pick);
  }
}

main();
