import { WidgetHoriz } from "./horiz.js";
import { Emoji, WidgetEmoji } from "./emoji.js";
import { WidgetText } from "./text.js";

export class WidgetTodo extends WidgetHoriz {
  #stars: WidgetEmoji[];
  #text: WidgetText;
  #check: WidgetEmoji;

  constructor() {
    super();

    this.#stars = [];

    for (let i = 0; i < 5; ++ i) {
      const emoji = new WidgetEmoji(Emoji.circleBlack);
      const clickCount = i + 1;
      emoji.handleClick(() => {
        this.setStars(clickCount);
      });
      this.append(emoji);
      this.#stars.push(emoji);
    }

    this.setStars(((Math.random() * 5)|0) + 1);

    this.#text = new WidgetText("");
    this.append(this.#text);

    this.#check = new WidgetEmoji(Emoji.check)
    this.append(this.#check);
    this.#check.handleClick(() => this.check());
  }

  check() {
    this.setStars(1);
    this.#text.set("");
  }

  setStars(count: number) {
    let idx = 0;
    for (const star of this.#stars) {
      ++idx;
      if (idx <= count) {
        star.set(Emoji.star);
      } else {
        star.set(Emoji.circleBlack);
      }
    }
  }
}
