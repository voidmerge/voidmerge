import { WidgetCont } from "./cont.js";

export class WidgetVert extends WidgetCont {
  constructor() {
    super();
    this.getElem().className = "widget-vert";
  }
}
