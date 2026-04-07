import { createContext, useCallback, useContext, useEffect, useMemo, useState, type PropsWithChildren } from 'react';
import { flushSync } from 'react-dom';
import {
  DEFAULT_LOCALE,
  DEFAULT_THEME,
  LOCALE_STORAGE_KEY,
  THEME_STORAGE_KEY,
  normalizeLocale,
  normalizeTheme,
  translate,
  type Locale,
  type ThemePreference,
  type TranslationKey,
} from './i18n';

interface AppPreferencesValue {
  locale: Locale;
  theme: ThemePreference;
  isDark: boolean;
  setLocale: (locale: Locale) => void;
  setTheme: (theme: ThemePreference, event?: React.MouseEvent | MouseEvent) => void;
  t: (key: TranslationKey) => string;
}

const AppPreferencesContext = createContext<AppPreferencesValue | null>(null);

const resolveIsDark = (theme: ThemePreference) => {
  if (theme === 'auto') {
    return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
      ? !window.matchMedia('(prefers-color-scheme: light)').matches
      : true;
  }
  return theme === 'dark';
};

export function AppPreferencesProvider({ children }: PropsWithChildren) {
  const [locale, setLocaleState] = useState<Locale>(() => normalizeLocale(localStorage.getItem(LOCALE_STORAGE_KEY)));
  const [theme, setThemeState] = useState<ThemePreference>(() => normalizeTheme(localStorage.getItem(THEME_STORAGE_KEY)));
  const isDark = useMemo(() => resolveIsDark(theme), [theme]);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.add('theme-transitioning');
    root.setAttribute('data-theme', isDark ? 'dark' : 'light');
    root.lang = locale;
    const timer = window.setTimeout(() => root.classList.remove('theme-transitioning'), 850);
    return () => window.clearTimeout(timer);
  }, [isDark, locale]);

  useEffect(() => {
    const media = typeof window.matchMedia === 'function'
      ? window.matchMedia('(prefers-color-scheme: light)')
      : null;
    if (!media) {
      return;
    }
    const handleChange = () => {
      if (theme === 'auto') {
        setThemeState('auto');
      }
    };
    media.addEventListener('change', handleChange);
    return () => media.removeEventListener('change', handleChange);
  }, [theme]);

  useEffect(() => {
    const handleStorage = (event: StorageEvent) => {
      if (event.key === LOCALE_STORAGE_KEY) {
        setLocaleState(normalizeLocale(event.newValue));
      }
      if (event.key === THEME_STORAGE_KEY) {
        setThemeState(normalizeTheme(event.newValue));
      }
    };
    window.addEventListener('storage', handleStorage);
    return () => window.removeEventListener('storage', handleStorage);
  }, []);

  const setLocale = useCallback((nextLocale: Locale) => {
    setLocaleState(nextLocale);
    localStorage.setItem(LOCALE_STORAGE_KEY, nextLocale);
  }, []);

  const setTheme = useCallback((nextTheme: ThemePreference, event?: React.MouseEvent | MouseEvent) => {
    // Check for View Transitions API support
    const isTransitionSupported = typeof document !== 'undefined' && 'startViewTransition' in document;
    
    if (event && isTransitionSupported) {
      const x = ('clientX' in event) ? event.clientX : innerWidth / 2;
      const y = ('clientY' in event) ? event.clientY : innerHeight / 2;
      
      const endRadius = Math.hypot(
        Math.max(x, innerWidth - x),
        Math.max(y, innerHeight - y)
      );

      // Cast document to any since TS might not have startViewTransition
      const transition = (document as any).startViewTransition(() => {
        flushSync(() => {
          setThemeState(nextTheme);
        });
        localStorage.setItem(THEME_STORAGE_KEY, nextTheme);
      });

      transition.ready.then(() => {
        document.documentElement.animate(
          {
            clipPath: [
              `circle(0px at ${x}px ${y}px)`,
              `circle(${endRadius}px at ${x}px ${y}px)`,
            ],
          },
          {
            duration: 800,
            easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
            pseudoElement: '::view-transition-new(root)',
          }
        );
      });
    } else {
      setThemeState(nextTheme);
      localStorage.setItem(THEME_STORAGE_KEY, nextTheme);
    }
  }, []);

  const value: AppPreferencesValue = {
    locale,
    theme,
    isDark,
    setLocale,
    setTheme,
    t: (key) => translate(locale, key),
  };

  return <AppPreferencesContext.Provider value={value}>{children}</AppPreferencesContext.Provider>;
}

export function useAppPreferences() {
  const context = useContext(AppPreferencesContext);
  if (!context) {
    throw new Error('AppPreferencesProvider is required');
  }
  return context;
}

export const defaultLocale = DEFAULT_LOCALE;
export const defaultTheme = DEFAULT_THEME;
