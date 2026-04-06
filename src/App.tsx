import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import { AppPreferencesProvider } from './lib/app-preferences';

export default function App() {
  return (
    <AppPreferencesProvider>
      <BrowserRouter>
        <Routes>
          <Route path="*" element={<Layout />} />
        </Routes>
      </BrowserRouter>
    </AppPreferencesProvider>
  );
}
