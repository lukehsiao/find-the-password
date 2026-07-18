import { test, expect } from "@playwright/test";

test("homepage shows the challenge title and intro", async ({ page }) => {
  await page.goto("http://localhost:3000/");

  await expect(page).toHaveTitle("Challenge: Find the Password");
  await expect(page.locator("h1#finding-the-password")).toHaveText(
    "Finding the password",
  );
});

test("joining the challenge lands on the user page", async ({ page }) => {
  // Unique per run so reruns against a live server don't collide.
  const username = "e2e" + Date.now();

  await page.goto("http://localhost:3000/");
  await page.fill('input[name="username"]', username);
  await page.click('input[type="submit"]');

  await expect(page).toHaveURL(`http://localhost:3000/u/${username}`);
  await expect(page.locator("h1#username")).toHaveText(`Hi, ${username}!`);
  await expect(
    page.locator('a[download="passwords.txt"]'),
  ).toBeVisible();
});

test("confirming a wrong password shows an error, not a solve", async ({
  page,
}) => {
  // Unique per run so reruns against a live server don't collide.
  const username = "confirm" + Date.now();

  await page.goto("http://localhost:3000/");
  await page.fill('input[name="username"]', username);
  await page.click('input[type="submit"]');
  await expect(page).toHaveURL(`http://localhost:3000/u/${username}`);

  await page.fill('input[name="password"]', "definitely-wrong");
  await page.click('input[type="submit"]');

  await expect(page.locator(".error p")).toHaveText(
    "That's not the password. Keep hunting!",
  );

  // The wrong guess counted as an attempt but did not solve.
  await page.goto("http://localhost:3000/");
  const row = page.locator("tr", { hasText: username });
  await expect(row).toHaveCount(1);
  await expect(row.locator("td").nth(1)).toHaveText("no");
});

test("rapid confirmation attempts hit the cooldown", async ({ page }) => {
  // Unique per run so reruns against a live server don't collide.
  const username = "throttle" + Date.now();

  await page.goto("http://localhost:3000/");
  await page.fill('input[name="username"]', username);
  await page.click('input[type="submit"]');
  await expect(page).toHaveURL(`http://localhost:3000/u/${username}`);

  // The first wrong guess is evaluated and arms the 10s window; the second
  // lands well inside it and must be rejected without being evaluated.
  await page.fill('input[name="password"]', "wrong-one");
  await page.click('input[type="submit"]');
  await expect(page.locator(".error p")).toHaveText(
    "That's not the password. Keep hunting!",
  );
  await page.fill('input[name="password"]', "wrong-two");
  await page.click('input[type="submit"]');
  await expect(page.locator(".error p")).toHaveText(
    "Whoa, slow down! You can only confirm once every 10 seconds.",
  );
});

test("the roster lists a registered player with their attempt count", async ({
  page,
}) => {
  // Unique per run so reruns against a live server don't collide.
  const username = "roster" + Date.now();

  await page.goto("http://localhost:3000/");
  await page.fill('input[name="username"]', username);
  await page.click('input[type="submit"]');
  await expect(page).toHaveURL(`http://localhost:3000/u/${username}`);

  for (let i = 0; i < 3; i++) {
    const res = await page.request.get(
      `http://localhost:3000/u/${username}/check/wrongpassword`,
    );
    expect(await res.text()).toBe("false");
  }

  // An unsolved player is absent from the leaderboard but shows up in the
  // roster with every attempt counted.
  await page.goto("http://localhost:3000/");
  const row = page.locator("tr", { hasText: username });
  await expect(row).toHaveCount(1);
  await expect(row.locator("code")).toHaveText("3");
});
