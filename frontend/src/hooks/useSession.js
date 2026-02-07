import { useEffect, useState } from "react";

export function useSession({ apiBase }) {
  const [userEmail, setUserEmail] = useState("");

  useEffect(() => {
    const loadSession = async () => {
      try {
        const response = await fetch(`${apiBase}/api/session`);
        if (!response.ok) {
          return;
        }
        const data = await response.json();
        if (data?.userEmail) {
          setUserEmail(data.userEmail);
        }
      } catch (err) {
        // Ignore session fetch errors.
      }
    };
    loadSession();
  }, [apiBase]);

  return userEmail;
}
