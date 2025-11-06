import { WidgetPage } from "./page.js";
import { WidgetMain } from "./main.js";
import { WidgetHoriz } from "./horiz.js";
import { WidgetAvatar } from "./avatar.js";
import { WidgetLabel } from "./label.js";
import { WidgetLeagueItem } from "./league-item.js";
import { Emoji, WidgetEmoji } from "./emoji.js";
import type { MainState } from "../state.ts";
import { pkToShort, Ident } from "../ident.js";

export class WidgetLeague extends WidgetHoriz {
  #page: WidgetPage;
  #main: WidgetMain;
  #update: () => void;
  #state: MainState;
  #ident: Ident;
  #back: WidgetEmoji;

  constructor(
    page: WidgetPage,
    main: WidgetMain,
    ident: Ident,
    state: MainState,
  ) {
    super();

    this.setWrap(true);

    this.#page = page;
    this.#main = main;

    this.#update = () => {};
    this.#state = state;

    this.#ident = ident;

    this.#back = new WidgetEmoji(Emoji.back);
    this.#back.getElem().classList.add("pin-top");
    this.#back.handleClick(() => this.#page.setChild(this.#main));

    this.render();
  }

  render() {
    this.clear();
    this.append(this.#back);

    const league: any = [
      [this.#ident.pk(), this.#state.starCount, this.#ident.avatarCode()],
    ];
    for (const item of this.#state.leagueData.leagues[this.#state.league - 1]) {
      if (item[0] !== this.#ident.pk()) {
        league.push(item);
      }
    }
    league.sort((a: any, b: any) => b[1] - a[1]);
    for (const [id, stars, avatarCode] of league) {
      const item = new WidgetLeagueItem(pkToShort(id), avatarCode, stars);
      if (id === this.#ident.pk()) {
        item.setIsSelf(true);
      }
      this.append(item);
    }
  }

  setUpdate(update: () => void) {
    this.#update = () => {
      this.render();
      update();
    };
  }
}
