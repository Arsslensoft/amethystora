// When Today offers Your year (R7-30), with no GNOME in it, for build_files/shared/test-your-year.mjs: from the middle of
// December to the middle of January, about the year ending, once, and to an account at least two months old.

const OLD_ENOUGH_DAYS = 60;

export function offeredYear(now, bornSeconds, seenYear) {
    const month = now.getMonth();
    const day = now.getDate();
    let year = null;
    if (month === 11 && day >= 15)
        year = now.getFullYear();
    else if (month === 0 && day <= 15)
        year = now.getFullYear() - 1;
    if (year === null || year === seenYear)
        return null;
    if (bornSeconds > 0 && now.getTime() / 1000 - bornSeconds < OLD_ENOUGH_DAYS * 86400)
        return null;
    return year;
}
