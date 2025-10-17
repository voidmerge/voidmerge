import { MainState, getWeekId } from "./state.js";
import { Ident } from "./ident.js";
import { WidgetPage, WidgetLoading, WidgetMain } from "./widgets.js";

const page = new WidgetPage();
page.setChild(new WidgetLoading());

function debounce(cb: () => void): () => void {
  let timer: any = undefined;
  return () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      clearTimeout(timer);
      cb();
    }, 1000);
  };
}

const STATE = "TodoLeaderState";

function loadState(): MainState {
  const raw = JSON.parse(localStorage.getItem(STATE) || "{}");
  const out: MainState = {
    promoted: false,
    weekId: "",
    starCount: 0,
    league: 1,
    todo: [],
  };

  if (
    raw &&
    typeof raw === "object" &&
    Array.isArray(raw.todo) &&
    typeof raw.promoted === "boolean" &&
    typeof raw.weekId === "string" &&
    typeof raw.starCount === "number" &&
    typeof raw.league === "number"
  ) {
    out.promoted = raw.promoted;
    out.weekId = raw.weekId;
    out.starCount = raw.starCount;
    out.league = raw.league;
    for (const todo of raw.todo) {
      if (
        typeof todo === "object" &&
        typeof todo.stars === "number" &&
        typeof todo.todo === "string"
      ) {
        out.todo.push({ stars: todo.stars, todo: todo.todo });
      }
    }
  }

  while (out.todo.length < 5) {
    out.todo.push({ stars: 1, todo: "" });
  }

  if (out.weekId.length < 1) {
    out.weekId = getWeekId();
  }

  const curWeekId = getWeekId();
  if (out.weekId !== curWeekId) {
    out.weekId = curWeekId;
    const tgt = out.league * 5;
    if (out.starCount >= tgt) {
      out.league += 1;
      out.promoted = true;
    }
    if (out.starCount < tgt - 5) {
      out.league -= 1;
    }
    if (out.league < 1) {
      out.league = 1;
    }
    out.starCount = 0;
  }

  return out;
}

const state = loadState();

function saveState() {
  localStorage.setItem(STATE, JSON.stringify(state));
}

saveState();

async function sync() {
  console.log("sync");

  const myIdent: Ident =
    (await Ident.load()) ||
    (await (async () => {
      const tmp = await Ident.random();
      tmp.store();
      return tmp;
    })());

  console.log("loaded", myIdent.debug(), state);

  const main = new WidgetMain(myIdent, state);

  main.setUpdate(
    debounce(() => {
      saveState();
    }),
  );

  page.setChild(main);
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
