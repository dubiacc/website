// --- LATIN GRAMMAR TESTS

const courseData = {};

function toggleSection(id) {
    let section = document.getElementById(id);
    if (!section) {
        return;
    }
    if (section.classList.contains("hidden")) {
        section.classList.remove("hidden");
    } else {
        section.classList.add("hidden");
    }
}

// Levenshtein distance for tolerance.
function levenshtein(a, b) {
if(!a || !b) return (a || b).length;
let matrix = [];
let i;
for(i = 0; i <= b.length; i++) {
    matrix[i] = [i];
}
let j;
for(j = 0; j <= a.length; j++) {
    matrix[0][j] = j;
}
for(i = 1; i <= b.length; i++) {
    for(j = 1; j <= a.length; j++) {
    if(b.charAt(i - 1) === a.charAt(j - 1)) {
        matrix[i][j] = matrix[i - 1][j - 1];
    } else {
        matrix[i][j] = Math.min(
        matrix[i - 1][j - 1] + 1,
        Math.min(
            matrix[i][j - 1] + 1,
            matrix[i - 1][j] + 1
        )
        );
    }
    }
}
return matrix[b.length][a.length];
}

let allPlaceholders = [];

function reloadTest(lessonId) {
    const section = courseData.sections.find(s => s.id === lessonId);
    const testId = Math.floor(Math.random() * section.tests.length);
    const chosenTest =  section.tests[testId];
    let graderesult = document.getElementById(`grade-result-${lessonId}`);
    graderesult.dataset.activetest = "" + testId;
    renderTest(lessonId, chosenTest);
}

function renderTest(lessonId, test) {

    const container = document.getElementById(`test-sentences-${lessonId}`);
    container.innerHTML = '';

    // We'll reset the global placeholders array.
    allPlaceholders[lessonId] = [];

    test.sentences.forEach((sentence, sentenceIndex) => {

        // We'll create a list item.
        const li = document.createElement('li');
        const p = document.createElement('p');

        sentence.segments.forEach(segment => {
        if(typeof segment === 'string') {
            // Just a text fragment.
            p.appendChild(document.createTextNode(segment));
        } else {

            const pid = `input-${lessonId}-${test.id}-${segment.placeholderId}`;
            // It's a placeholder object.
            const input = document.createElement('input');
            input.id = pid;
            input.type = 'text';
            input.placeholder = segment.baseForm;

            // We'll store a reference to the correct forms and explanation.
            allPlaceholders[lessonId][pid] = {
            answers: segment.answers,
            explanation: segment.explanation
            };

            p.appendChild(input);
        }
        });

        li.appendChild(p);
        container.appendChild(li);
    });

}

function gradeTest(lessonId) {

    const resultDiv = document.getElementById(`grade-result-${lessonId}`);
    let testId = parseInt(resultDiv.dataset.activetest);
    let correctCount = 0;
    let totalCount = 0;

    for(let placeholderId in allPlaceholders[lessonId]) {
        const input = document.getElementById(`input-${lessonId}-${testId}-${placeholderId}`);
        if(!input) continue;

        totalCount++;

        const userValue = input.value.trim();
        const {answers, explanation} = allPlaceholders[placeholderId];

        let isCorrect = false;
        let bestDistance = Infinity;
        let bestAnswer = "";

        for(const ans of answers) {
        const distance = levenshtein(userValue.toLowerCase(), ans.toLowerCase());
        if(distance < bestDistance) {
            bestDistance = distance;
            bestAnswer = ans;
        }
        // If near match (distance <=1) or exact match:
        if(userValue.toLowerCase() === ans.toLowerCase()) {
            isCorrect = true;
            break;
        }
        }

        if(isCorrect) {
        input.classList.remove('incorrect');
        input.classList.add('correct');
        removeSolutionHint(input);
        correctCount++;
        } else {
        input.classList.remove('correct');
        input.classList.add('incorrect');
        showSolutionHint(input, bestAnswer, explanation);
        }
    }

    const percentage = Math.round((correctCount / totalCount) * 100);
    const gradeLetter = calcGrade(percentage);
    resultDiv.textContent = `You got ${correctCount} out of ${totalCount} correct (${percentage}%). Grade: ${gradeLetter}`;
}

function calcGrade(percentage) {
let gradeLetter = 'F';
if(percentage >= 90) {
    gradeLetter = 'A';
} else if(percentage >= 80) {
    gradeLetter = 'B';
} else if(percentage >= 70) {
    gradeLetter = 'C';
} else if(percentage >= 60) {
    gradeLetter = 'D';
} else if(percentage >= 50) {
    gradeLetter = 'E';
} 
return gradeLetter;
}

function showSolutionHint(inputElement, correctSolution, explanation) {
removeSolutionHint(inputElement);

const hint = document.createElement('span');
hint.className = 'solution-hint';

const solutionText = document.createTextNode(`→ ${correctSolution} `);
hint.appendChild(solutionText);

const questionLink = document.createElement('span');
questionLink.className = 'explanation-link';
questionLink.textContent = '?';
questionLink.onclick = function() {
    alert(explanation);
};
hint.appendChild(questionLink);

inputElement.insertAdjacentElement('afterend', hint);
}

function removeSolutionHint(inputElement) {
    const nextElem = inputElement.nextElementSibling;
    if(nextElem && nextElem.classList.contains('solution-hint')) {
        nextElem.remove();
    }
}

// TODO: for tests in testData, render tests
window.onload = function() { 
    for (let i = 0; i < COURSE_DATA_LEN; i++) {
        reloadTest(i);
    }
};

