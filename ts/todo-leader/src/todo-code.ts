import * as VM from "@voidmerge/voidmerge-code";
import assets from "./assets.js";
import { b64Dec } from "./b64.js";
import { avatarSvg } from "./avatar.js";

const LEAGUES = [
  [
    [
      "BA1LTqE1prQHQ3KgftiH2SPvvLEXYkj73r1vVSsdeXg",
      "0",
      "45ceIsQ-NPZvw22YPmg-CA",
    ],
    [
      "puJqtHk3ToFnq5JDTNvlawFASomtpnZeoIaXlISA18E",
      "1",
      "xad62qW5V1dczb0w7EwR6g",
    ],
    [
      "LtW73psJ6ChknTVAjInbR8bkHb7gXbVYszOmJnf1aNw",
      "2",
      "BtkcuopHI3VepsOULvjS7w",
    ],
  ],
  [
    [
      "S62gC4pGarKKXUeMYz4UT-k_vefNtK84u6NXmTDAMRY",
      "5",
      "Rdo_P46mK0reCGxuU16Hjw",
    ],
    [
      "aShdEtMaG7PqNA8RW-7aTyCLDUaDgmOiIf09UqEPgKs",
      "6",
      "d5xkB7xm-CWh_dbBpxIi4Q",
    ],
    [
      "dEOHQjOYnE7BGQFf5Q6S4XKCqg5o-rNfKAAkZt-R844",
      "7",
      "9aJ4mv_VgMwuMlAhMoH1yQ",
    ],
  ],
  [
    [
      "4bBMMKbfqDGoM2Y3YZr7jeyjI35RcC8yCWMvC2IpCPM",
      "10",
      "Cm5Zfw3Isi-UaA37RvtRbw",
    ],
    [
      "eWujb64TEhCJRhXrckrB3aoe4L8mKVC_45cE7YhkZXU",
      "11",
      "mqAQ-iMAq8MNeJohvVKjlA",
    ],
    [
      "2wZEKm5w7v-2EXpwkKStmO0LU7hkv3zKu06ICqGkyds",
      "12",
      "BGzmt5c_x_ORCSqdQfYrPA",
    ],
  ],
  [
    [
      "XbY5YjTnTiqmahHnjG79ZNPQ58od8Zaaqfi9SrbkOT8",
      "15",
      "fwjAQvyxfeJmJQI7ay1OyA",
    ],
    [
      "VAws8ww7owYiizE-y4Gnc_2QtUOuuGjKEbMBFrlFDos",
      "16",
      "3RJonp0GBGiSkeK8B5-cIw",
    ],
    [
      "qzue4stDtEwqKvnSkm66nIhcyY64BGIvwxp8ZcXJcEA",
      "17",
      "_kPm8uxJoGj30Vq9YL2ZMg",
    ],
  ],
  [
    [
      "vCRcS-jl3MYC37v1fbu-Jw-GRFz09GLbaPMdwnKpQCc",
      "20",
      "hA1nPW4lG3T8hIEHxpOHCQ",
    ],
    [
      "rPLu8T1Oa77uJzzuhhawI9t4d2lsb76rmWNjdKeLvjw",
      "21",
      "bhMjby1jTUFCBJ6ZmphYQQ",
    ],
    [
      "dNKlaJ_TfaOdq-6YfHN9r6rUs-Jd2x00gv9xqx5JX8U",
      "22",
      "P94bH36lJwwdghoPmEKLWw",
    ],
  ],
  [
    [
      "0jIQC3MIhQfL36dx-jsV95IsPx9DkgYDIbvJN0vi2G0",
      "25",
      "ebPuo9YazTf8QHpl2nHx0w",
    ],
    [
      "7iyVPaV6valGN_GgYCcKmGs_s7CZ-6sOOtsiuzo_ezc",
      "26",
      "pzTE2Krj8ooPILSzDDry_g",
    ],
    [
      "-dim7VaIVrD-EbfYAiErMZaN5Y4BDiXjoXnQ9PoWzwk",
      "27",
      "gQvpPKL9pSASBp6K_oLSBA",
    ],
  ],
  [
    [
      "mHo03scwQdIKnrI6ByV-VqJTqQrtVvkqk3dkiSY9yJk",
      "30",
      "Dqz3nRrB9bU-wQQ4n0VVnA",
    ],
    [
      "WH6P2QWJpcJcLVmaTQt-zZxHVpDthP42RQ5TiOgWQpg",
      "31",
      "E0WCeiz7mRdLha4By9f3bA",
    ],
    [
      "Ikc6j7NTQiLcRXUnuXw8VyxPQ5NiqWA4EPqlsiuIFMI",
      "32",
      "KUeVN9yqrtSsu1xCTq55sA",
    ],
  ],
  [
    [
      "BAFakT848-eLrFuCoyJzOZlxmJtDV2YaZiz8cbxt4Ro",
      "35",
      "gP8lt9XEGulFFlo6uGOAQQ",
    ],
    [
      "3hAKs3wtARca8dm2wuCrsrZgquuutDPE8BzpLhex17U",
      "36",
      "NOR6c25OmNXipcfI6ETrig",
    ],
    [
      "GaEOf3UoBDQ7raWSd9ZzpurU45G9MWxUERcvJDPgwOk",
      "37",
      "VKJ7ShWuOBw_YekPBF_Zag",
    ],
  ],
  [
    [
      "z-WaDdMgm86HxrL-ytyTZZAa7TxwBIpMwBvxMmUHTXc",
      "40",
      "i4lEeWiUvF972b1CkJzdHA",
    ],
    [
      "ZvsCrJL9rbs_7XbbyIpTs8iYwB7PBobL2ZB-_fcFajE",
      "41",
      "hte8fW2ybzHnI3iDVzrE0Q",
    ],
    [
      "jL_dbupVl5QmMc6vq23cfXhSVW7bOIlqT7g8ZG9bJtA",
      "42",
      "5MoyDlgSi1oRLpxYu6rngg",
    ],
  ],
];

/**
 * Main dispatch handler.
 */
VM.defineVoidMergeHandler(async (req) => {
  const type = req.type;
  if (req instanceof VM.RequestCodeConfig) {
    return new VM.ResponseCodeConfigOk({
      cronIntervalSecs: 60.0 * 10.0, // 10 minutes
    });
  } else if (req instanceof VM.RequestCron) {
    return await handleCron();
  } else if (req instanceof VM.RequestObjCheck) {
    return await handleObjCheck(req);
  } else if (req instanceof VM.RequestFn) {
    return await handleFn(req);
  } else {
    throw new Error(`invalid request type: ${type}`);
  }
});

/**
 * Handle our periodic maintenance tasks.
 */
async function handleCron(): Promise<VM.ResponseCronOk> {
  let newest = 0;
  let userCount = 0;
  const leagues: any = JSON.parse(JSON.stringify(LEAGUES));

  while (true) {
    const { metaList } = await VM.objList({
      appPathPrefix: "stars~",
      createdGt: newest,
      limit: 100,
    });

    if (metaList.length < 1) {
      break;
    }

    userCount += metaList.length;

    for (const meta of metaList) {
      if (meta.createdSecs() > newest) {
        newest = meta.createdSecs();
      }
      const pk = meta.appPath().substring(6);
      const { data } = await VM.objGet({ meta });
      const parsed = JSON.parse(new TextDecoder().decode(data));
      const weekId = parsed.shift();
      const league = parseInt(parsed.shift());
      const starCount = parsed.shift();
      const sig = parsed.shift();
      const avatarCode = parsed.shift();

      leagues[league - 1].push([pk, starCount, avatarCode]);

      if (leagues[league - 1].length > 6) {
        leagues[league - 1].shift();
      }
    }
  }

  const { meta } = await VM.objPut({
    meta: VM.ObjMeta.fromParts({
      appPath: "agg",
      // 1 week
      expiresSecs: Date.now() / 1000 + 60 * 60 * 24 * 7,
    }),
    data: new TextEncoder().encode(
      JSON.stringify({
        userCount,
        leagues,
      }),
    ),
  });

  return new VM.ResponseCronOk();
}

/**
 * Validate data to be added to the object store.
 */
async function handleObjCheck(
  req: VM.RequestObjCheck,
): Promise<VM.ResponseObjCheckOk> {
  return new VM.ResponseObjCheckOk();
}

/**
 * Handle user requests.
 */
async function handleFn(req: VM.RequestFn): Promise<VM.ResponseFnOk> {
  if (req.path === "cron") {
    await handleCron();
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode("Ok"),
      headers: {
        "content-type": "text/plain",
      },
    });
  } else if (req.path === "env") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(JSON.stringify(VM.env())),
      headers: {
        "content-type": "application/json",
      },
    });
  } else if (req.path === "agg") {
    let res = { state: "no-data" };

    try {
      const { data } = await VM.objGet({
        meta: VM.ObjMeta.fromParts({
          appPath: "agg",
        }),
      });
      const tmp = JSON.parse(new TextDecoder().decode(data));
      if (tmp && typeof tmp === "object") {
        res = tmp;
        res.state = "success";
      }
    } catch (e: any) {
      /* pass */
    }
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(JSON.stringify(res)),
      headers: {
        "content-type": "application/json",
      },
    });
  } else if (req.path === "publish") {
    const raw = JSON.parse(new TextDecoder().decode(req.body));
    if (!Array.isArray(raw)) {
      throw new Error("publish body must be an array");
    }
    const path = raw.shift();
    if (typeof path !== "string" || !path.startsWith("stars~")) {
      throw new Error("invalid path");
    }

    const data = new TextEncoder().encode(JSON.stringify(raw));

    const { meta } = await VM.objPut({
      meta: VM.ObjMeta.fromParts({
        appPath: path,
        // 1 week
        expiresSecs: Date.now() / 1000 + 60 * 60 * 24 * 7,
      }),
      data,
    });

    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(meta.fullPath()),
      headers: {
        "content-type": "application/json",
      },
    });
  } else if (req.path === "favicon.svg") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(assets["favicon.svg"]),
      headers: {
        "content-type": "image/svg+xml",
      },
    });
  } else if (req.path === "index.css") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(assets["index.css"]),
      headers: {
        "content-type": "text/css",
      },
    });
  } else if (req.path === "index.js") {
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(assets["index.js"]),
      headers: {
        "content-type": "application/javascript",
      },
    });
  } else if (req.path.startsWith("avatar/")) {
    const avatarCode = b64Dec(req.path.split("/")[1]);
    if (avatarCode.byteLength !== 16) {
      throw new Error("invalid avatar code");
    }
    const svg = avatarSvg(avatarCode);
    return new VM.ResponseFnOk({
      status: 200,
      body: new TextEncoder().encode(svg),
      headers: {
        "content-type": "image/svg+xml",
      },
    });
  }

  return new VM.ResponseFnOk({
    status: 200,
    body: new TextEncoder().encode(assets["index.html"]),
    headers: {
      "content-type": "text/html",
    },
  });
}
