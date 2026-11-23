/**
 * Static export for GitHub Pages.
 *
 * GitHub Pages serves a project site under /<repo-name>/, so the base path is
 * read from PAGES_BASE_PATH (set by the deploy workflow). Locally it is empty,
 * so `npm run dev` serves at http://localhost:3001/.
 */
const basePath = process.env.PAGES_BASE_PATH || "";

/** @type {import('next').NextConfig} */
const nextConfig = {
  output: "export",
  basePath,
  trailingSlash: true,
  images: { unoptimized: true },
  reactStrictMode: true,
  env: {
    NEXT_PUBLIC_BASE_PATH: basePath,
  },
};

export default nextConfig;
