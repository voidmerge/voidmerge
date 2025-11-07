export interface TodoState {
  stars: number;
  todo: string;
}

export interface MainState {
  promoted: boolean;
  weekId: string;
  starCount: number;
  league: number;
  leagueData: any;
  lastLeagueUpdate: number;
  todo: TodoState[];
}

export function getWeekId(): string {
  // current time
  const now = new Date();

  // get the weekday of jan 1st
  const offset = new Date(Date.UTC(now.getUTCFullYear())).getDay();

  // adjust for weekday offset, this may be a negative number,
  // but js will correct it by winding back the year
  now.setUTCDate(now.getUTCDate() - offset);

  // get the current adjusted year
  const year = now.getUTCFullYear();

  // get origin again, because the year may have been backset
  const origin = new Date(Date.UTC(year));

  // get the number of weeks since the year start
  const week =
    ((now.getTime() - origin.getTime()) / 1000 / 60 / 60 / 24 / 7) | 0;

  // return the unique week id
  return `${year}-${week}`;
}
