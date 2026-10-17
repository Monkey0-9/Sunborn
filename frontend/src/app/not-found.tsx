export default function NotFound() {
  return (
    <div className="flex flex-col items-center justify-center min-h-[60vh] text-center">
      <h2 className="text-2xl font-bold mb-4">404 - Page Not Found</h2>
      <p className="text-gray-400 mb-6">The requested resource could not be found.</p>
      <a href="/" className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded">
        Return Home
      </a>
    </div>
  );
}
