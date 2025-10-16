import { Ident } from "./ident.js";
import { WidgetPage, WidgetLoading, WidgetMain } from "./widgets.js";

const page = new WidgetPage();
page.setChild(new WidgetLoading());

async function sync() {
  console.log("sync");

  const myIdent: Ident =
    (await Ident.load()) ||
    (await (async () => {
      const tmp = await Ident.random();
      tmp.store();
      return tmp;
    })());

  console.log("loaded", myIdent.debug());

  page.setChild(new WidgetMain(myIdent));
}

let lastSync: number = 0;
function checkAnim(curTimestamp: number) {
  requestAnimationFrame(checkAnim);

  if (curTimestamp - lastSync > 1000 * 60 * 10) {
    lastSync = curTimestamp;
    setTimeout(sync, 0);
  }
}
setInterval(() => {
  requestAnimationFrame(checkAnim);
}, 10000);
requestAnimationFrame(checkAnim);
sync();
