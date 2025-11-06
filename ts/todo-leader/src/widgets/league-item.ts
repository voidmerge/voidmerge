import { WidgetHoriz } from "./horiz.js";
import { WidgetVert } from "./vert.js";
import { WidgetAvatar } from "./avatar.js";
import { WidgetLabel } from "./label.js";
import { Emoji, WidgetEmoji } from "./emoji.js";

export class WidgetLeagueItem extends WidgetHoriz {
  constructor(short: string, avatarCode: string, starCount: string) {
    super();

    this.getElem().classList.add("league-item");

    this.setExpand(false);

    this.append(new WidgetAvatar(avatarCode));

    let vert = new WidgetVert();
    this.append(vert);

    vert.append(new WidgetLabel(short));

    let horiz = new WidgetHoriz();
    vert.append(horiz);

    horiz.append(new WidgetEmoji(Emoji.star));
    horiz.append(new WidgetLabel(starCount));
  }

  setIsSelf(v: boolean) {
    this.getElem().classList.toggle("is-self", v);
  }
}
