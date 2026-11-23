export const REPO_URL = "https://github.com/Monkey0-9/Sunborn";
export const blob = (path: string) => `${REPO_URL}/blob/main/${path}`;
/** "" locally, "/Sunborn" on GitHub Pages. Plain anchors need it; next/link adds it itself. */
export const BASE_PATH = process.env.NEXT_PUBLIC_BASE_PATH || "";
